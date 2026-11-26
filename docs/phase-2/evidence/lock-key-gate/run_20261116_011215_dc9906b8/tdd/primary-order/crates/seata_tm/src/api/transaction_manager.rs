use crate::{
    GlobalTransaction, GlobalTransactionRole, RootContext, TmError, TmOperation, TransactionConfig,
    TransactionOutcome, Xid,
};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::{
    GlobalBeginRequest, GlobalCommitRequest, GlobalRollbackRequest, GlobalStatusRequest,
};
use seata_protocol::{Message, ResultCode};
use seata_transport::{
    CallError, ConnectionPool, ConnectionPoolConfig, RegistrationIdentity, SessionRegistration,
    TransportError,
};
use std::time::Duration;
use tokio::time::Instant;
/// 拥有原生TM登记连接池的正式事务管理器；所有业务调用只发送一次。
/// 对应 Java: org.apache.seata.tm.DefaultTransactionManager，重试边界按已批准ADR-0008。
pub struct TransactionManager {
    pool: ConnectionPool,
    request_timeout: Duration,
    pub(crate) recovery_authority: std::sync::Arc<()>,
}
/// TransactionManager伴随构建器，显式分离网络等待与TC事务期限。
/// 对应 Java: TM客户端配置及DefaultTransactionManager初始化。
#[derive(Debug)]
pub struct TransactionManagerBuilder {
    connection: ConnectionPoolConfig,
    identity: RegistrationIdentity,
    request_timeout: Duration,
    registration_timeout: Duration,
}
impl TransactionManager {
    /// 从原Commit/Rollback未知错误恢复受限句柄，不创建新事务或新TC期限。
    /// 仅接受本管理器实际Launcher签发的私有凭证；伪造/改写/其他管理器错误均不能授权。
    /// Begin/Status错误不授权结束，重试前仍须reconcile。
    pub fn recover_unknown(&self, error: &TmError) -> Result<GlobalTransaction<'_>, TmError> {
        let identity = match error {
            TmError::UnknownOutcome {
                operation,
                xid: Some(xid),
                recovery: Some(proof),
                ..
            }
            | TmError::ProtocolFailure {
                operation,
                xid: Some(xid),
                may_have_effect: true,
                recovery: Some(proof),
                ..
            } if matches!(operation, TmOperation::Commit | TmOperation::Rollback)
                && proof.matches(&self.recovery_authority, xid, *operation) =>
            {
                Some((*operation, xid.clone()))
            }
            _ => None,
        };
        let (operation, xid) = identity.ok_or_else(|| TmError::IllegalState {
            xid: error.xid().cloned(),
            reason: "only an original unknown end outcome can authorize recovery",
        })?;
        Ok(GlobalTransaction::recover(self, xid, operation))
    }
    /// 按六传播策略执行业务工厂；Launcher成功提交/失败回滚各至多一次，保留二阶段错误。
    /// panic/取消只恢复本地上下文，不启动无所有者的网络任务；TC负责持久超时恢复。
    pub async fn execute<T, E, F: std::future::Future<Output = Result<T, E>>>(
        &self,
        config: TransactionConfig,
        business: impl FnOnce() -> F,
    ) -> Result<crate::TransactionResult<T>, crate::TransactionError<E>> {
        crate::TransactionalTemplate::new(self)
            .execute(config, business)
            .await
    }
    /// 参数为连接预算和精确TM登记身份；返回默认5秒请求/登记预算的构建器。
    pub fn builder(
        connection: ConnectionPoolConfig,
        identity: RegistrationIdentity,
    ) -> TransactionManagerBuilder {
        TransactionManagerBuilder {
            connection,
            identity,
            request_timeout: Duration::from_secs(5),
            registration_timeout: Duration::from_secs(5),
        }
    }
    /// 创建一次Launcher；返回TC真实XID及响应字段，不隐式绑定调用任务，不自动重试。
    pub async fn begin(
        &self,
        config: &TransactionConfig,
    ) -> Result<GlobalTransaction<'_>, TmError> {
        if RootContext::current().is_some() {
            return Err(TmError::IllegalState {
                xid: RootContext::current().map(|c| c.xid().clone()),
                reason: "explicit Begin requires a suspended or empty context",
            });
        }
        let deadline = Instant::now()
            .checked_add(config.transaction_timeout())
            .ok_or(TmError::InvalidConfiguration {
                reason: "transaction deadline overflow",
            })?;
        let response = self
            .rpc(
                TmOperation::Begin,
                None,
                Message::GlobalBeginRequest(GlobalBeginRequest {
                    timeout: config.timeout_millis(),
                    transaction_name: Some(config.name().to_owned()),
                }),
            )
            .await?;
        let Some(Message::GlobalBeginResponse(value)) = response.body else {
            return Err(Self::protocol(
                TmOperation::Begin,
                None,
                "expected GlobalBeginResponse",
            ));
        };
        if value.response.result_code == ResultCode::Failed {
            return Err(TmError::DefiniteRejected {
                operation: TmOperation::Begin,
                xid: None,
                response: Box::new(value.response.clone()),
                status: None,
                reported_begin: Some(Box::new(value)),
            });
        }
        let xid = value
            .xid
            .and_then(|value| Xid::parse(value).ok())
            .ok_or_else(|| {
                Self::protocol(
                    TmOperation::Begin,
                    None,
                    "successful Begin requires a valid returned XID",
                )
            })?;
        Ok(GlobalTransaction::new(
            self,
            xid,
            GlobalTransactionRole::Launcher,
            Some(deadline),
            Some(value.response),
            value.extra_data,
        ))
    }
    /// 参数为外部已验证XID；返回只参与、不发送Begin或全局二阶段的句柄。
    pub fn participant(&self, xid: Xid) -> GlobalTransaction<'_> {
        GlobalTransaction::new(
            self,
            xid,
            GlobalTransactionRole::Participant,
            None,
            None,
            None,
        )
    }
    /// 查询参数XID一次；保留返回状态/结果前缀，不自动轮询或推断Finished为提交成功。
    pub async fn get_status(&self, xid: &Xid) -> Result<TransactionOutcome, TmError> {
        self.status_call(TmOperation::Status, xid).await
    }
    pub(crate) async fn status_call(
        &self,
        operation: TmOperation,
        xid: &Xid,
    ) -> Result<TransactionOutcome, TmError> {
        let identity = Some(xid.as_str().to_owned());
        let body = match operation {
            TmOperation::Commit => Message::GlobalCommitRequest(GlobalCommitRequest {
                xid: identity,
                extra_data: None,
            }),
            TmOperation::Rollback => Message::GlobalRollbackRequest(GlobalRollbackRequest {
                xid: identity,
                extra_data: None,
            }),
            TmOperation::Status => Message::GlobalStatusRequest(GlobalStatusRequest {
                xid: identity,
                extra_data: None,
            }),
            _ => {
                return Err(TmError::IllegalState {
                    xid: Some(xid.clone()),
                    reason: "status call requires an end or query operation",
                })
            }
        };
        let reply = self.rpc(operation, Some(xid.clone()), body).await?;
        let (response, status) = match reply.body {
            Some(Message::GlobalCommitResponse(value)) if operation == TmOperation::Commit => {
                (value.response, value.global_status)
            }
            Some(Message::GlobalRollbackResponse(value)) if operation == TmOperation::Rollback => {
                (value.response, value.global_status)
            }
            Some(Message::GlobalStatusResponse(value)) if operation == TmOperation::Status => {
                (value.response, value.global_status)
            }
            _ => {
                return Err(Self::protocol(
                    operation,
                    Some(xid.clone()),
                    "unexpected status response family",
                ))
            }
        };
        if response.result_code == ResultCode::Failed {
            Err(TmError::DefiniteRejected {
                operation,
                xid: Some(xid.clone()),
                response: Box::new(response),
                status: Some(status),
                reported_begin: None,
            })
        } else {
            Ok(TransactionOutcome { response, status })
        }
    }
    async fn rpc(
        &self,
        operation: TmOperation,
        xid: Option<Xid>,
        body: Message,
    ) -> Result<RpcMessage, TmError> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(TmError::from_call(
                operation,
                xid,
                CallError::LocalFailureBeforeSend {
                    generation: None,
                    request_id: None,
                    reason: TransportError::InvalidConfiguration {
                        reason: "Tokio I/O and time runtime required",
                    },
                },
            ));
        }
        self.pool
            .handle()
            .request(body, self.request_timeout)
            .await
            .map_err(|source| TmError::from_call(operation, xid, source))
    }
    fn protocol(operation: TmOperation, xid: Option<Xid>, reason: &'static str) -> TmError {
        TmError::ProtocolFailure {
            operation,
            xid,
            may_have_effect: true,
            reason,
            source: None,
            recovery: None,
        }
    }
    /// 消费管理器并关闭其自有TM池，等待监督任务结束；不替任何事务发送二阶段。
    pub async fn shutdown(self) -> Result<(), TmError> {
        self.pool
            .shutdown()
            .await
            .map_err(|e| TmError::Infrastructure(Box::new(e)))
    }
}
impl TransactionManagerBuilder {
    /// 参数是单次网络调用总预算，包含获取连接；返回构建器，不改变TC事务超时。
    pub fn request_timeout(mut self, value: Duration) -> Self {
        self.request_timeout = value;
        self
    }
    /// 参数是每代际TM身份登记预算；返回构建器。
    pub fn registration_timeout(mut self, value: Duration) -> Self {
        self.registration_timeout = value;
        self
    }
    /// 验证网络预算并启动仅TM角色的登记池；需要Tokio I/O/time运行时，不发送业务事务。
    pub fn build(self) -> Result<TransactionManager, TmError> {
        if self.request_timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(self.request_timeout)
                .is_none()
        {
            return Err(TmError::InvalidConfiguration {
                reason: "positive finite transport request timeout required",
            });
        }
        let registration = SessionRegistration::tm(self.identity, self.registration_timeout)
            .map_err(|e| TmError::Infrastructure(Box::new(e)))?;
        let pool = ConnectionPool::start_registered(self.connection, registration)
            .map_err(|e| TmError::Infrastructure(Box::new(e)))?;
        Ok(TransactionManager {
            pool,
            request_timeout: self.request_timeout,
            recovery_authority: std::sync::Arc::new(()),
        })
    }
}
impl std::fmt::Debug for TransactionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransactionManager")
            .field("pool", &self.pool.handle().snapshot())
            .field("request_timeout", &self.request_timeout)
            .finish()
    }
}
