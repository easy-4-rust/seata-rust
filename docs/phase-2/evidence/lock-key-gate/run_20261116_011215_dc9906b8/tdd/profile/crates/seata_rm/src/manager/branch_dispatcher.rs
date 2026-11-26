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
}
impl BranchDispatcher {
    /// 参数为目录及共享持久日志；返回不拥有网络重试调度的资源分派器。
    pub fn new(catalog: ResourceCatalog, store: FileBranchStore) -> Self {
        Self { catalog, store }
    }
    /// 参数为原始身份和TC方向；返回Java分支状态；错误不能解释为资源未执行。
    /// 取消/适配器错误保留Pending；相同完成请求不再调用资源，恢复Unknown不重放。
    pub async fn dispatch(
        &self,
        identity: BranchIdentity,
        phase: BranchPhase,
    ) -> Result<BranchStatus, RmError> {
        let resource = self.catalog.lookup(identity.resource())?;
        let mut lease = self.store.lease(identity.clone(), phase).await?;
        let resolution = match lease.state() {
            Some(BranchRecordState::Completed) => {
                return Ok(status(phase, BranchResolution::Applied))
            }
            Some(BranchRecordState::Rejected) => {
                return Ok(status(phase, BranchResolution::Rejected))
            }
            Some(BranchRecordState::Pending) => {
                match resource.recover(&identity, phase).await? {
                    // NotApplied必须围栏已取消的旧尝试；适配器承担此资源级证明。
                    BranchResolution::NotApplied => resource.apply(&identity, phase).await?,
                    other => other,
                }
            }
            None | Some(BranchRecordState::RetrySafe) => {
                lease.mark_pending().await?;
                resource.apply(&identity, phase).await?
            }
        };
        match resolution {
            BranchResolution::Applied => lease.mark_completed().await?,
            BranchResolution::Rejected => lease.mark_rejected().await?,
            BranchResolution::NotApplied => lease.mark_retry_safe().await?,
            BranchResolution::Unknown => {}
        }
        Ok(status(phase, resolution))
    }
}
fn status(phase: BranchPhase, resolution: BranchResolution) -> BranchStatus {
    match (phase, resolution) {
        (BranchPhase::Commit, BranchResolution::Applied) => BranchStatus::PhaseTwoCommitted,
        (BranchPhase::Rollback, BranchResolution::Applied) => BranchStatus::PhaseTwoRollbacked,
        (BranchPhase::Commit, BranchResolution::Rejected) => {
            BranchStatus::PhaseTwoCommitFailedUnretryable
        }
        (BranchPhase::Rollback, BranchResolution::Rejected) => {
            BranchStatus::PhaseTwoRollbackFailedUnretryable
        }
        (BranchPhase::Commit, _) => BranchStatus::PhaseTwoCommitFailedRetryable,
        (BranchPhase::Rollback, _) => BranchStatus::PhaseTwoRollbackFailedRetryable,
    }
}
