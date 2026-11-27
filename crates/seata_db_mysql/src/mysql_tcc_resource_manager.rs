use crate::{
    mysql_tcc_fence_store::MySqlTccFenceStore, mysql_tcc_transaction::MySqlTccTransaction, DbError,
    MySqlDataSource, MySqlTccTryError,
};
use seata_protocol::{transaction::BranchRegisterRequest, BranchType};
use seata_rm::{
    BranchCallbackObserver, BranchCallbackReceipt, BranchIdentity, BranchPhase, BranchResolution,
    BranchResource, RegisteredBranch, ResourceClient, RmError,
};
use seata_tcc::{
    BusinessActionContext, CommonFenceStatus, FenceDecision, FenceHandler, FenceOperation,
    TccBusinessAction,
};
use seata_transport::ResourceKey;
use std::{fmt, future::Future, pin::Pin, sync::Arc};
use tokio::time::{timeout_at, Instant};
/// 同库持久 Fence 的 TCC 三阶段资源，不把回调日志或缺行当作业务完成。
/// 对应 Java: org.apache.seata.rm.tcc.TCCResourceManager 与 org.apache.seata.rm.fence.SpringFenceHandler。
pub struct MySqlTccResourceManager {
    source: MySqlDataSource,
    key: ResourceKey,
    action: Arc<dyn TccBusinessAction>,
}
impl fmt::Debug for MySqlTccResourceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlTccResourceManager")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}
impl MySqlTccResourceManager {
    /// 参数为原数据源、精确动作名及三阶段业务对象；返回不自动建表、登记或清理 Fence 的资源。
    pub fn new(
        source: MySqlDataSource,
        action_name: String,
        action: Arc<dyn TccBusinessAction>,
    ) -> Result<Self, DbError> {
        if action_name.len() > 512 {
            return Err(DbError::Input {
                reason: "TCC action name exceeds bound",
            });
        }
        let key = ResourceKey::new(BranchType::Tcc, action_name).map_err(|_| DbError::Input {
            reason: "invalid TCC action name",
        })?;
        Ok(Self {
            source,
            key,
            action,
        })
    }
    fn context(
        &self,
        xid: &str,
        branch: i64,
        application_data: Option<&str>,
    ) -> Result<BusinessActionContext, DbError> {
        if xid.is_empty()
            || xid.len() > 128
            || xid.trim() != xid
            || xid.chars().any(char::is_control)
        {
            return Err(DbError::Input {
                reason: "TCC XID must be original bounded UTF-8",
            });
        }
        let context = BusinessActionContext::new(
            Some(xid.into()),
            branch,
            Some(self.key.resource_id().into()),
            application_data,
        )
        .map_err(|_| DbError::Input {
            reason: "TCC application context rejected",
        })?;
        if !context.use_common_fence() {
            return Err(DbError::Input {
                reason: "MySQL TCC requires explicit Fence true",
            });
        }
        Ok(context)
    }
    fn identity_context(&self, id: &BranchIdentity) -> Result<BusinessActionContext, DbError> {
        if id.resource() != &self.key {
            return Err(DbError::RecoveryConflict);
        }
        self.context(id.xid(), id.branch_id(), id.application_data())
    }
    /// 参数为正式客户端、原 XID 与原应用信封；单次登记后同事务执行 Try，任何错误保留已知原回执。
    /// 不自动重发登记、Try、AT PhaseOne_Done 或动态上下文报告。
    pub async fn register_and_try(
        &self,
        client: &ResourceClient,
        xid: &str,
        application_data: Option<String>,
    ) -> Result<RegisteredBranch, MySqlTccTryError> {
        self.context(xid, 0, application_data.as_deref())
            .map_err(|source| MySqlTccTryError::Local { source })?;
        let deadline = Instant::now() + self.source.operation_timeout();
        let mut known = None;
        let mut started = false;
        let mut commit_started = false;
        let result = timeout_at(deadline, async {
            started = true;
            let receipt = client
                .branch_register_with_receipt(BranchRegisterRequest {
                    xid: Some(xid.into()),
                    branch_type: BranchType::Tcc,
                    resource_id: Some(self.key.resource_id().into()),
                    lock_key: None,
                    application_data,
                })
                .await?;
            known = Some(Box::new(receipt.clone()));
            self.try_inner(&receipt, &mut commit_started)
                .await
                .map_err(|source| {
                    MySqlTccTryError::transaction(receipt.clone(), source, commit_started)
                })?;
            Ok(receipt)
        })
        .await;
        if Instant::now() >= deadline {
            return Err(MySqlTccTryError::Deadline {
                receipt: known,
                registration_may_have_effect: started,
                try_commit_may_have_effect: commit_started,
            });
        }
        result.map_err(|_| MySqlTccTryError::Deadline {
            receipt: known,
            registration_may_have_effect: started,
            try_commit_may_have_effect: commit_started,
        })?
    }
    /// 参数为正式客户端签发的原成功回执；任何已有 Fence 都拒绝重复 Try，不重造身份。
    pub async fn try_registered(
        &self,
        receipt: RegisteredBranch,
    ) -> Result<RegisteredBranch, MySqlTccTryError> {
        let deadline = Instant::now() + self.source.operation_timeout();
        let mut commit_started = false;
        let result = timeout_at(deadline, self.try_inner(&receipt, &mut commit_started)).await;
        if Instant::now() >= deadline {
            return Err(MySqlTccTryError::Deadline {
                receipt: Some(Box::new(receipt)),
                registration_may_have_effect: false,
                try_commit_may_have_effect: commit_started,
            });
        }
        result
            .map_err(|_| MySqlTccTryError::Deadline {
                receipt: Some(Box::new(receipt.clone())),
                registration_may_have_effect: false,
                try_commit_may_have_effect: commit_started,
            })?
            .map_err(|source| {
                MySqlTccTryError::transaction(receipt.clone(), source, commit_started)
            })?;
        Ok(receipt)
    }
    async fn try_inner(
        &self,
        receipt: &RegisteredBranch,
        commit_started: &mut bool,
    ) -> Result<(), DbError> {
        let id = receipt.identity();
        let context = self.identity_context(id)?;
        if receipt.lock_key().is_some() {
            return Err(DbError::RecoveryConflict);
        }
        let mut tx = MySqlTccTransaction::begin(&self.source).await?;
        let state = MySqlTccFenceStore::read(tx.control()?, &self.source, id).await?;
        if FenceHandler::plan(FenceOperation::Try, state)
            != FenceDecision::InsertAndInvoke(CommonFenceStatus::Tried)
        {
            return Err(DbError::RecoveryConflict);
        }
        MySqlTccFenceStore::insert(tx.control()?, &self.source, id, CommonFenceStatus::Tried)
            .await?;
        if !self
            .action
            .try_action(&context, &mut tx)
            .await
            .map_err(|_| DbError::Input {
                reason: "TCC Try action failed",
            })?
        {
            return Err(DbError::Input {
                reason: "TCC Try action refused",
            });
        }
        tx.commit_tracked(commit_started).await
    }
    async fn finish(
        &self,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchResolution, DbError> {
        let context = self.identity_context(id)?;
        let mut tx = MySqlTccTransaction::begin(&self.source).await?;
        let state = MySqlTccFenceStore::read(tx.control()?, &self.source, id).await?;
        let operation = match phase {
            BranchPhase::Commit => FenceOperation::Confirm,
            BranchPhase::Rollback => FenceOperation::Cancel,
        };
        match FenceHandler::plan(operation, state) {
            FenceDecision::AlreadyApplied => {
                tx.commit().await?;
                Ok(BranchResolution::Applied)
            }
            FenceDecision::UpdateAndInvoke { expected, next } => {
                // 持原行锁排除旧尝试；状态与业务在一个事务中生效，未知提交只能再核对原 Fence。
                MySqlTccFenceStore::update(tx.control()?, id, expected, next).await?;
                let result = match phase {
                    BranchPhase::Commit => self.action.confirm(&context, &mut tx).await,
                    BranchPhase::Rollback => self.action.cancel(&context, &mut tx).await,
                };
                if !result.map_err(|_| DbError::Input {
                    reason: "TCC phase-two action failed",
                })? {
                    return Err(DbError::Input {
                        reason: "TCC phase-two action refused",
                    });
                }
                tx.commit().await?;
                Ok(BranchResolution::Applied)
            }
            // 缺行 Cancel 只能由原连接 observer 唯一插入屏障；手工 apply/recover 不授予该权限。
            FenceDecision::InsertSuspended | FenceDecision::MissingTry | FenceDecision::Refuse => {
                Ok(BranchResolution::Unknown)
            }
            FenceDecision::InsertAndInvoke(_) => Err(DbError::RecoveryConflict),
        }
    }
    async fn resolve(
        &self,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchResolution, RmError> {
        let deadline = Instant::now() + self.source.operation_timeout();
        let result = timeout_at(deadline, self.finish(id, phase)).await;
        if Instant::now() >= deadline {
            return Ok(BranchResolution::Unknown);
        }
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(DbError::RecoveryConflict)) => Err(RmError::IdentityConflict),
            _ => Ok(BranchResolution::Unknown),
        }
    }
    async fn observe_rollback(&self, receipt: &BranchCallbackReceipt) -> Result<(), DbError> {
        self.identity_context(receipt.identity())?;
        let mut tx = MySqlTccTransaction::begin(&self.source).await?;
        if MySqlTccFenceStore::read(tx.control()?, &self.source, receipt.identity())
            .await?
            .is_none()
        {
            MySqlTccFenceStore::insert(
                tx.control()?,
                &self.source,
                receipt.identity(),
                CommonFenceStatus::Suspended,
            )
            .await?;
        }
        tx.commit().await
    }
}
impl BranchResource for MySqlTccResourceManager {
    fn callback_observer(&self) -> Option<&dyn BranchCallbackObserver> {
        Some(self)
    }
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn apply<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(self.resolve(identity, phase))
    }
    fn recover<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(self.resolve(identity, phase))
    }
}
impl BranchCallbackObserver for MySqlTccResourceManager {
    fn observe<'a>(
        &'a self,
        receipt: &'a BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), RmError>> + Send + 'a>> {
        Box::pin(async move {
            if receipt.phase() != BranchPhase::Rollback {
                return Ok(());
            }
            let deadline = Instant::now() + self.source.operation_timeout();
            let result = timeout_at(deadline, self.observe_rollback(receipt)).await;
            if Instant::now() >= deadline {
                return Err(RmError::Unavailable {
                    reason: "TCC callback Fence deadline",
                });
            }
            result
                .map_err(|_| RmError::Unavailable {
                    reason: "TCC callback Fence deadline",
                })?
                .map_err(|e| match e {
                    DbError::RecoveryConflict => RmError::IdentityConflict,
                    _ => RmError::Unavailable {
                        reason: "TCC callback Fence unresolved",
                    },
                })
        })
    }
}
