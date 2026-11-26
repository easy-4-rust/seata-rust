use crate::{
    DurableGlobalTransaction, DurableTmError, GlobalTransaction, GlobalTransactionRole,
    LauncherOrigin, LauncherStore, RootContext, TmError, TmOperation, TransactionConfig,
    TransactionOutcome, Xid,
};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::{
    GlobalBeginRequest, GlobalCommitRequest, GlobalRollbackRequest, GlobalStatusRequest,
};
use seata_protocol::{Message, ResultCode};
use seata_transport::{
    CallError, ConnectionPool, ConnectionPoolConfig, PoolSampler, PoolSnapshot,
    RegistrationIdentity, SessionRegistration, TransportError,
};
use std::time::Duration;
use tokio::time::Instant;
/// 拥有原生TM登记连接池的正式事务管理器；所有业务调用只发送一次。
/// 对应 Java: org.apache.seata.tm.DefaultTransactionManager，重试边界按已批准ADR-0008。
pub struct TransactionManager {
    pool: ConnectionPool,
    metrics: super::TmMetricsSampler,
    request_timeout: Duration,
    tc_endpoint: std::net::SocketAddr,
    identity: RegistrationIdentity,
    owner_instance_id: [u8; 32],
    owner_stop_scope: Option<[u8; 32]>,
    pub(crate) recovery_authority: std::sync::Arc<()>,
}
/// TransactionManager伴随构建器，显式分离网络等待与TC事务期限。
/// 对应 Java: TM客户端配置及DefaultTransactionManager初始化。
#[derive(Debug)]
pub struct TransactionManagerBuilder {
    #[cfg(feature = "tls")]
    transport_tls: Option<seata_transport::TlsClientConfig>,
    connection: ConnectionPoolConfig,
    identity: RegistrationIdentity,
    request_timeout: Duration,
    registration_timeout: Duration,
    owner_stop_scope: Option<[u8; 32]>,
}
impl TransactionManager {
    /// 无参数；返回只读调用计数采样器，关闭后仍可读取最终累计值。
    /// 它不授予网络、关闭或事务恢复权限；计数不跨管理器或进程聚合。
    pub fn metrics_sampler(&self) -> super::TmMetricsSampler {
        self.metrics.clone()
    }

    /// 返回此管理器的随机实例 ID；供受信进程监护器把停止证据与 008 来源对应，ID 本身不授予恢复权。
    pub const fn owner_instance_id(&self) -> &[u8; 32] {
        &self.owner_instance_id
    }
    /// 返回此管理器持久事务来源所使用的停止证明信任域。
    pub fn owner_stop_scope(&self) -> Option<&[u8; 32]> {
        self.owner_stop_scope.as_ref()
    }
    /// 向监护器的专用输出报告本管理器实例 ID 并刷新输出，供进程退出后核验。
    pub fn announce_supervised_instance(
        &self,
        mut writer: impl std::io::Write,
    ) -> std::io::Result<()> {
        writer.write_all(super::launcher_process_monitor::INSTANCE_PREFIX.as_bytes())?;
        for byte in self.owner_instance_id {
            write!(writer, "{byte:02x}")?;
        }
        writer.write_all(b"\n")?;
        writer.flush()
    }
    /// 参数为事务配置、最长 256 字节的业务意图及持久层；仅在 Begin 来源确认落盘后返回 Launcher。
    /// 来源写入失败时即使 TC 已创建事务也不交付句柄，交由 TC 超时恢复。
    pub async fn begin_durable<'a, S: LauncherStore>(
        &'a self,
        config: &TransactionConfig,
        business_intent: &str,
        store: &'a S,
    ) -> Result<DurableGlobalTransaction<'a, S>, DurableTmError> {
        if business_intent.is_empty() || business_intent.len() > 256 {
            return Err(TmError::InvalidConfiguration {
                reason: "nonempty business intent up to 256 bytes required",
            }
            .into());
        }
        let started = Instant::now();
        let inner = self.begin(config).await?;
        let remaining = config
            .transaction_timeout()
            .saturating_sub(started.elapsed());
        if remaining.as_micros() == 0 {
            return Err(TmError::TransactionDeadlineExceeded {
                xid: inner.xid().clone(),
            }
            .into());
        }
        let response = inner
            .begin_response()
            .ok_or_else(|| TmError::IllegalState {
                xid: Some(inner.xid().clone()),
                reason: "successful Begin response missing",
            })?;
        let origin = LauncherOrigin {
            xid: inner.xid().clone(),
            tc_endpoint: self.tc_endpoint,
            application_id: self.identity.application_id().to_owned(),
            service_group: self.identity.transaction_service_group().to_owned(),
            begin_exception_code: response.transaction_exception_code,
            begin_extra_data: inner.extra_data().map(str::to_owned),
            business_intent: business_intent.to_owned(),
            remaining,
            captured_at: Instant::now(),
            owner_instance_id: self.owner_instance_id,
            owner_stop_scope: self.owner_stop_scope,
        };
        store
            .record_begin(&origin)
            .await
            .map_err(|source| DurableTmError::Persistence {
                stage: "begin source",
                xid: origin.xid().clone(),
                source,
            })?;
        Ok(DurableGlobalTransaction {
            inner,
            store,
            origin,
        })
    }
    /// 返回所拥有 TM 连接池的只读槽位、当前代际及池生命周期累计计数；不获取业务通道。
    /// 返回值中的连接计数与池状态分别采样，不能当作跨锁原子瞬时值。
    pub fn connection_snapshot(&self) -> PoolSnapshot {
        self.pool.handle().snapshot()
    }
    /// 返回可由观测服务持有的只读池采样能力；不授予事务或网络操作权限。
    pub fn connection_sampler(&self) -> PoolSampler {
        self.pool.handle().sampler()
    }
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
            #[cfg(feature = "tls")]
            transport_tls: None,
            connection,
            identity,
            request_timeout: Duration::from_secs(5),
            registration_timeout: Duration::from_secs(5),
            owner_stop_scope: None,
        }
    }
    /// 创建一次Launcher；返回TC真实XID及响应字段，不隐式绑定调用任务，不自动重试。
    pub async fn begin(
        &self,
        config: &TransactionConfig,
    ) -> Result<GlobalTransaction<'_>, TmError> {
        let guard = self.metrics.begin(TmOperation::Begin);
        let result = self.begin_inner(config).await;
        guard.finish(&result);
        result
    }

    async fn begin_inner(
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
    /// 受信持久存储确认新代际的发送标记后，发送原 Commit 或 Rollback 一次，原样返回 TC 回包或错误。
    pub async fn send_recovered_end(
        &self,
        authority: &crate::RecoveredEndAuthority,
        gate: impl crate::RecoveredEndGate,
    ) -> Result<TransactionOutcome, DurableTmError> {
        if !std::sync::Arc::ptr_eq(&self.recovery_authority, &authority.0) {
            return Err(TmError::IllegalState {
                xid: None,
                reason: "recovered end authority does not match manager",
            }
            .into());
        }
        let (xid, operation) = gate.arm(self).await?;
        if !matches!(operation, TmOperation::Commit | TmOperation::Rollback) {
            return Err(TmError::IllegalState {
                xid: Some(xid),
                reason: "recovered end requires commit or rollback",
            }
            .into());
        }
        // 直接调用单次 RPC，持久门闩接管原内存 Launcher 的未知结果权限。
        self.status_call(operation, &xid).await.map_err(Into::into)
    }
    /// 核对来源的 TC 端点、应用和服务组后，使用当前登记连接查询原 XID 一次。
    /// 返回 TC 原状态；身份不符时在发送查询前返回错误。
    /// 对应 Java: DefaultTransactionManager#getStatus，额外核对持久来源身份。
    pub async fn get_status_at_origin(
        &self,
        xid: &Xid,
        tc_endpoint: std::net::SocketAddr,
        application_id: &str,
        service_group: &str,
    ) -> Result<TransactionOutcome, TmError> {
        if self.tc_endpoint != tc_endpoint
            || self.identity.application_id() != application_id
            || self.identity.transaction_service_group() != service_group
        {
            return Err(TmError::IllegalState {
                xid: Some(xid.clone()),
                reason: "TM endpoint or registration does not match launcher origin",
            });
        }
        self.get_status(xid).await
    }
    pub(crate) async fn status_call(
        &self,
        operation: TmOperation,
        xid: &Xid,
    ) -> Result<TransactionOutcome, TmError> {
        let guard = self.metrics.begin(operation);
        let result = self.status_call_inner(operation, xid).await;
        guard.finish(&result);
        result
    }

    async fn status_call_inner(
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
        match self.pool.handle().request(body, self.request_timeout).await {
            Ok(reply) => {
                // 收到报文只代表传输有权威响应；ResultCode仍由上层解析。
                tracing::info!(target: "seata_tm::rpc", operation = ?operation, xid = ?xid.as_ref().map(Xid::as_str), request_id = reply.id, outcome = "response", "tm_rpc_outcome");
                Ok(reply)
            }
            Err(source) => {
                let request_id = source.request_id();
                let error = TmError::from_call(operation, xid, source);
                let outcome = if error.is_unknown_outcome() {
                    "unknown"
                } else {
                    "not_sent"
                };
                // 仅记录静态结果类别与关联身份，不输出请求体、错误Debug或凭据。
                tracing::warn!(target: "seata_tm::rpc", operation = ?operation, xid = ?error.xid().map(Xid::as_str), request_id = ?request_id, outcome, "tm_rpc_outcome");
                Err(error)
            }
        }
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
    /// 参数为显式 mTLS 配置；返回每个连接代际都验证相同 TLS 身份的构建器，不回退明文。
    #[cfg(feature = "tls")]
    pub fn transport_tls(mut self, tls: seata_transport::TlsClientConfig) -> Self {
        self.transport_tls = Some(tls);
        self
    }
    /// 指定持久事务来源的部署信任域。此值须与本节点停止证明签发方配置一致。
    pub fn launcher_stop_scope(mut self, scope: [u8; 32]) -> Self {
        self.owner_stop_scope = Some(scope);
        self
    }
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
        let mut owner_instance_id = [0_u8; 32];
        getrandom::getrandom(&mut owner_instance_id).map_err(|_| {
            TmError::InvalidConfiguration {
                reason: "operating system random source required for TM instance identity",
            }
        })?;
        let tc_endpoint = self.connection.endpoint();
        let identity = self.identity.clone();
        let registration = SessionRegistration::tm(self.identity, self.registration_timeout)
            .map_err(|e| TmError::Infrastructure(Box::new(e)))?;
        let pool = {
            #[cfg(feature = "tls")]
            {
                match self.transport_tls {
                    Some(tls) => {
                        ConnectionPool::start_registered_tls(self.connection, registration, tls)
                    }
                    None => ConnectionPool::start_registered(self.connection, registration),
                }
            }
            #[cfg(not(feature = "tls"))]
            {
                ConnectionPool::start_registered(self.connection, registration)
            }
        }
        .map_err(|e| TmError::Infrastructure(Box::new(e)))?;
        Ok(TransactionManager {
            pool,
            metrics: super::TmMetricsSampler::new(),
            request_timeout: self.request_timeout,
            tc_endpoint,
            identity,
            owner_instance_id,
            owner_stop_scope: self.owner_stop_scope,
            recovery_authority: std::sync::Arc::new(()),
        })
    }

    /// 显式构建恢复控制方使用的 TM 与该实例独有的结束发送权限。
    /// 将权限与受信持久存储一起交给恢复控制方，普通 TM 调用方仅持有管理器。
    pub fn build_with_recovered_end_authority(
        self,
    ) -> Result<(TransactionManager, crate::RecoveredEndAuthority), TmError> {
        let manager = self.build()?;
        let authority = super::recovered_end_authority::RecoveredEndAuthority(
            manager.recovery_authority.clone(),
        );
        Ok((manager, authority))
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
