use crate::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResolution, FileBranchStore,
    ResourceCatalog, RmError,
};
use seata_protocol::BranchStatus;

/// 有界共享租约下的二阶段资源分派：先持久Pending，未决先恢复，完成先同步再返回。
/// 对应 Java: AbstractRMHandler#doBranchCommit/doBranchRollback与DefaultResourceManager分派。
#[derive(Debug, Clone)]
pub struct BranchDispatcher {
    catalog: ResourceCatalog,
    store: FileBranchStore,
    metrics: super::RmMetricsSampler,
}
impl BranchDispatcher {
    /// 参数为目录及共享持久日志；返回不拥有网络重试调度的资源分派器。
    pub fn new(catalog: ResourceCatalog, store: FileBranchStore) -> Self {
        let metrics = store.metrics_sampler();
        Self {
            catalog,
            store,
            metrics,
        }
    }
    /// 参数为原始身份和TC方向；返回Java分支状态；错误不能解释为资源未执行。
    /// 取消/适配器错误保留Pending；相同完成请求不再调用资源，恢复Unknown不重放。
    pub async fn dispatch(
        &self,
        identity: BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchStatus, RmError> {
        self.dispatch_inner(identity, phase, None).await
    }
    /// 参数为整批校验后的原连接回调能力；返回持久分支状态，公共手工入口不能构造此能力。
    pub(crate) async fn dispatch_callback(
        &self,
        receipt: crate::BranchCallbackReceipt,
    ) -> Result<BranchStatus, RmError> {
        self.dispatch_inner(receipt.identity().clone(), receipt.phase(), Some(&receipt))
            .await
    }
    async fn dispatch_inner(
        &self,
        identity: BranchIdentity,
        phase: BranchPhase,
        callback: Option<&crate::BranchCallbackReceipt>,
    ) -> Result<BranchStatus, RmError> {
        let mut guard = self.metrics.begin(phase);
        let result = self
            .dispatch_counted(identity, phase, callback, &mut guard)
            .await;
        guard.finish(&result);
        result
    }
    async fn dispatch_counted(
        &self,
        identity: BranchIdentity,
        phase: BranchPhase,
        callback: Option<&crate::BranchCallbackReceipt>,
        guard: &mut super::rm_dispatch_guard::RmDispatchGuard,
    ) -> Result<BranchStatus, RmError> {
        let resource = self.catalog.lookup(identity.resource())?;
        let journal_identity = if let Some(receipt) = callback {
            let stable = resource.callback_identity(receipt).await?;
            if stable.key() != identity.key() || stable.resource() != identity.resource() {
                return Err(RmError::IdentityConflict);
            }
            stable
        } else {
            identity.clone()
        };
        let mut lease = self.store.lease(journal_identity, phase).await?;
        let original_state = lease.state();
        match original_state {
            Some(BranchRecordState::Completed) => {
                if let Some(receipt) = callback {
                    let _ = receipt;
                }
                guard.reused(BranchRecordState::Completed);
                return Ok(status(phase, BranchResolution::Applied));
            }
            Some(BranchRecordState::Rejected) => {
                guard.reused(BranchRecordState::Rejected);
                return Ok(status(phase, BranchResolution::Rejected));
            }
            None | Some(BranchRecordState::RetrySafe) => {
                lease.mark_pending().await?;
            }
            Some(BranchRecordState::Pending) => {}
        }
        // 来源登记也是可取消的数据库控制动作；必须先持久 Pending，且仅原连接路径获得能力。
        if let Some(receipt) = callback {
            if let Some(observer) = resource.callback_observer() {
                observer.observe(receipt).await?;
            }
        }
        let resolution = match original_state {
            Some(BranchRecordState::Pending) => {
                guard.recover();
                match resource.recover(&identity, phase).await? {
                    // NotApplied必须围栏已取消的旧尝试；适配器承担此资源级证明。
                    BranchResolution::NotApplied => {
                        guard.apply();
                        resource.apply(&identity, phase).await?
                    }
                    other => other,
                }
            }
            None | Some(BranchRecordState::RetrySafe) => {
                guard.apply();
                resource.apply(&identity, phase).await?
            }
            Some(BranchRecordState::Completed | BranchRecordState::Rejected) => {
                return Err(RmError::Unavailable {
                    reason: "terminal branch cannot reenter dispatch",
                })
            }
        };
        match resolution {
            BranchResolution::Applied => lease.mark_completed().await?,
            BranchResolution::Rejected => lease.mark_rejected().await?,
            BranchResolution::NotApplied => lease.mark_retry_safe().await?,
            BranchResolution::Unknown | BranchResolution::ManualInterventionRequired => {}
        }
        guard.resolved(resolution);
        Ok(status(phase, resolution))
    }
}
fn status(phase: BranchPhase, resolution: BranchResolution) -> BranchStatus {
    match (phase, resolution) {
        (BranchPhase::Commit, BranchResolution::Applied) => BranchStatus::PhaseTwoCommitted,
        (BranchPhase::Rollback, BranchResolution::Applied) => BranchStatus::PhaseTwoRollbacked,
        (
            BranchPhase::Commit,
            BranchResolution::Rejected | BranchResolution::ManualInterventionRequired,
        ) => BranchStatus::PhaseTwoCommitFailedUnretryable,
        (
            BranchPhase::Rollback,
            BranchResolution::Rejected | BranchResolution::ManualInterventionRequired,
        ) => BranchStatus::PhaseTwoRollbackFailedUnretryable,
        (BranchPhase::Commit, _) => BranchStatus::PhaseTwoCommitFailedRetryable,
        (BranchPhase::Rollback, _) => BranchStatus::PhaseTwoRollbackFailedRetryable,
    }
}
