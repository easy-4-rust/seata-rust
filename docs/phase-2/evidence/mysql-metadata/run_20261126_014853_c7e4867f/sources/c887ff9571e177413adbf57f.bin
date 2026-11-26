use crate::{
    GlobalTransactionRole, RootContext, TmError, TmOperation, TransactionCompletion,
    TransactionContext, TransactionManager, TransactionOutcome, Xid,
};
use seata_protocol::transaction::AbstractTransactionResponse;
use seata_protocol::GlobalStatus;
use std::future::Future;
use tokio::time::Instant;
/// 原XID事务能力；参与方无全局结束权，未知结束只能先查询且不能切换操作。
/// 对应 Java: org.apache.seata.tm.api.DefaultGlobalTransaction；不复制Java本地重试循环。
pub struct GlobalTransaction<'a> {
    manager: &'a TransactionManager,
    xid: Xid,
    role: GlobalTransactionRole,
    deadline: Option<Instant>,
    begin_response: Option<AbstractTransactionResponse>,
    extra_data: Option<String>,
    state: EndState,
    last_status: GlobalStatus,
}
/// GlobalTransaction私有伴随状态；await前先保留不确定性，因此取消不会恢复为未发送。
/// 对应 Java: DefaultGlobalTransaction的status/role生命周期，Rust补充取消状态。
#[derive(Debug, Clone, Copy)]
enum EndState {
    Open,
    Uncertain {
        operation: TmOperation,
        reconciled: bool,
    },
    Acknowledged,
}
impl<'a> GlobalTransaction<'a> {
    pub(crate) fn recover(
        manager: &'a TransactionManager,
        xid: Xid,
        operation: TmOperation,
    ) -> Self {
        let mut transaction = Self::new(
            manager,
            xid,
            GlobalTransactionRole::Launcher,
            None,
            None,
            None,
        );
        transaction.state = EndState::Uncertain {
            operation,
            reconciled: false,
        };
        transaction.last_status = GlobalStatus::Unknown;
        transaction
    }
    pub(crate) fn new(
        manager: &'a TransactionManager,
        xid: Xid,
        role: GlobalTransactionRole,
        deadline: Option<Instant>,
        begin_response: Option<AbstractTransactionResponse>,
        extra_data: Option<String>,
    ) -> Self {
        Self {
            manager,
            xid,
            role,
            deadline,
            begin_response,
            extra_data,
            state: EndState::Open,
            last_status: GlobalStatus::Begin,
        }
    }
    /// 返回TC原样XID；未知结束后仍可用于显式恢复。
    pub const fn xid(&self) -> &Xid {
        &self.xid
    }
    /// 返回此句柄拥有的全局角色。
    pub const fn role(&self) -> GlobalTransactionRole {
        self.role
    }
    /// 返回Begin的原始可选附加数据，不把None变为空字符串。
    pub fn extra_data(&self) -> Option<&str> {
        self.extra_data.as_deref()
    }
    /// 返回实际Begin前缀；外部参与句柄未执行Begin，故返回None。
    pub const fn begin_response(&self) -> Option<&AbstractTransactionResponse> {
        self.begin_response.as_ref()
    }
    /// 返回是否仍保留一个无权威历史结果的结束操作；Finished不会清除此标记。
    pub const fn has_unknown_outcome(&self) -> bool {
        matches!(self.state, EndState::Uncertain { .. })
    }
    /// 参数是业务future工厂；在原XID作用域内构造/执行，返回业务结果，退出自动恢复外层。
    pub async fn scope<F: Future>(&self, business: impl FnOnce() -> F) -> F::Output {
        RootContext::scope_owned(Some(self.context()), async { business().await }).await
    }
    pub(crate) fn context(&self) -> TransactionContext {
        TransactionContext {
            xid: self.xid.clone(),
            role: self.role,
            deadline: self.deadline,
        }
    }
    /// 发起一次Commit，参与方直接返回Participant；未知结果不自动回滚或重发。
    pub async fn commit(&mut self) -> Result<TransactionCompletion, TmError> {
        self.end(TmOperation::Commit).await
    }
    /// 发起一次Rollback；已有未知Commit时本地拒绝，不向TC改变原操作。
    pub async fn rollback(&mut self) -> Result<TransactionCompletion, TmError> {
        self.end(TmOperation::Rollback).await
    }
    /// 查询同一原XID一次；保留Finished/重试超时，不创建本地重试调度器。
    pub async fn reconcile(&mut self) -> Result<TransactionOutcome, TmError> {
        // 每次查询先撤销上次一次性重试许可；查询失败或future取消均不得沿用旧许可。
        if let EndState::Uncertain { operation, .. } = self.state {
            self.state = EndState::Uncertain {
                operation,
                reconciled: false,
            };
        }
        let outcome = match self.manager.get_status(&self.xid).await {
            Ok(outcome) => outcome,
            Err(error) => {
                if let TmError::DefiniteRejected {
                    status: Some(status),
                    ..
                } = &error
                {
                    self.last_status = *status;
                }
                return Err(error);
            }
        };
        self.last_status = outcome.status();
        if let EndState::Uncertain { operation, .. } = self.state {
            self.state = if matches!(
                self.last_status,
                GlobalStatus::Committed
                    | GlobalStatus::CommitFailed
                    | GlobalStatus::Rollbacked
                    | GlobalStatus::RollbackFailed
                    | GlobalStatus::TimeoutRollbacked
                    | GlobalStatus::TimeoutRollbackFailed
            ) {
                EndState::Acknowledged
            } else {
                EndState::Uncertain {
                    operation,
                    reconciled: true,
                }
            };
        }
        Ok(outcome)
    }
    async fn end(&mut self, operation: TmOperation) -> Result<TransactionCompletion, TmError> {
        if self.role == GlobalTransactionRole::Participant {
            return Ok(TransactionCompletion::Participant(self.xid.clone()));
        }
        if operation == TmOperation::Commit
            && self
                .deadline
                .is_some_and(|deadline| deadline <= Instant::now())
        {
            return Err(TmError::TransactionDeadlineExceeded {
                xid: self.xid.clone(),
            });
        }
        let illegal = |reason| TmError::IllegalState {
            xid: Some(self.xid.clone()),
            reason,
        };
        if matches!(
            self.last_status,
            GlobalStatus::CommitRetryTimeout
                | GlobalStatus::RollbackRetryTimeout
                | GlobalStatus::StopCommitOrCommitRetry
                | GlobalStatus::StopRollbackOrRollbackRetry
        ) {
            return Err(illegal(
                "TC retry-timeout/stop requires operator intervention",
            ));
        }
        match self.state {EndState::Open=>{},EndState::Uncertain {operation:original,reconciled:true} if original==operation=>{},_=>return Err(illegal("end already acknowledged or unknown end requires same-operation status reconciliation"))}
        let previous = self.state;
        // 在任何网络await前保留原操作；future被取消时句柄仍要求显式协调。
        self.state = EndState::Uncertain {
            operation,
            reconciled: false,
        };
        match self.manager.status_call(operation, &self.xid).await {
            Ok(outcome) => {
                self.last_status = outcome.status();
                if self.last_status != GlobalStatus::Finished {
                    self.state = match previous {
                        EndState::Uncertain { operation, .. }
                            if !matches!(
                                self.last_status,
                                GlobalStatus::Committed
                                    | GlobalStatus::CommitFailed
                                    | GlobalStatus::Rollbacked
                                    | GlobalStatus::RollbackFailed
                                    | GlobalStatus::TimeoutRollbacked
                                    | GlobalStatus::TimeoutRollbackFailed
                            ) =>
                        {
                            EndState::Uncertain {
                                operation,
                                reconciled: false,
                            }
                        }
                        _ => EndState::Acknowledged,
                    };
                }
                Ok(TransactionCompletion::Observed {
                    xid: self.xid.clone(),
                    outcome,
                })
            }
            Err(mut error) => {
                if error.is_unknown_outcome() {
                    error.authorize_recovery(
                        &self.manager.recovery_authority,
                        self.xid.clone(),
                        operation,
                    );
                }
                if matches!(
                    error,
                    TmError::LocalFailureBeforeSend { .. }
                        | TmError::ProtocolFailure {
                            may_have_effect: false,
                            ..
                        }
                ) {
                    self.state = previous;
                } else if let TmError::DefiniteRejected { status, .. } = &error {
                    if let Some(status) = status {
                        self.last_status = *status;
                    }
                    self.state = match previous {
                        EndState::Uncertain { operation, .. }
                            if !matches!(
                                self.last_status,
                                GlobalStatus::Committed
                                    | GlobalStatus::CommitFailed
                                    | GlobalStatus::Rollbacked
                                    | GlobalStatus::RollbackFailed
                                    | GlobalStatus::TimeoutRollbacked
                                    | GlobalStatus::TimeoutRollbackFailed
                            ) =>
                        {
                            EndState::Uncertain {
                                operation,
                                reconciled: false,
                            }
                        }
                        _ => EndState::Acknowledged,
                    };
                }
                Err(error)
            }
        }
    }
}
impl std::fmt::Debug for GlobalTransaction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalTransaction")
            .field("xid", &self.xid)
            .field("role", &self.role)
            .field("state", &self.state)
            .field("last_status", &self.last_status)
            .field("extra_data_present", &self.extra_data.is_some())
            .finish_non_exhaustive()
    }
}
