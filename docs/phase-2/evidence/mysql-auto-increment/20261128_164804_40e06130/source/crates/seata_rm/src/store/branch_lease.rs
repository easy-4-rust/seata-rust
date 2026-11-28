use crate::{BranchIdentity, BranchPhase, BranchRecordState, FileBranchStore, RmError};
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedSemaphorePermit};

/// 持有共享日志中一个分支的排他执行权；取消后仍由在途磁盘任务保留租约及预算。
/// 对应 Java: RM分支串行处理语义；Rust恢复日志租约扩展。
#[derive(Debug)]
pub struct BranchLease {
    pub(crate) store: FileBranchStore,
    pub(crate) identity: BranchIdentity,
    pub(crate) phase: BranchPhase,
    pub(crate) state: Option<BranchRecordState>,
    pub(crate) guards: Arc<LeaseGuards>,
}

/// 租约内部伴随所有权；磁盘任务共享此权利直到真正结束，同一租约的取消写入也串行。
/// 对应 Java: RM分支互斥；Rust异步取消安全扩展。
#[derive(Debug)]
pub(crate) struct LeaseGuards {
    pub(crate) _key_permit: OwnedSemaphorePermit,
    pub(crate) _budget_permit: OwnedSemaphorePermit,
    pub(crate) operation: Arc<Mutex<()>>,
}
impl BranchLease {
    /// 返回最近一次已确认的日志状态；取消中的磁盘写入须由新租约重新读取。
    pub const fn state(&self) -> Option<BranchRecordState> {
        self.state
    }
    /// 返回本租约校验过的原始分支身份。
    pub fn identity(&self) -> &BranchIdentity {
        &self.identity
    }
    /// 在调用资源前持久记录可能执行；失败时不得调用资源。
    pub async fn mark_pending(&mut self) -> Result<(), RmError> {
        self.transition(BranchRecordState::Pending).await
    }
    /// 在资源权威确认已执行后同步完成状态；同步失败不得回复完成。
    pub async fn mark_completed(&mut self) -> Result<(), RmError> {
        self.transition(BranchRecordState::Completed).await
    }
    /// 仅当资源确认无效果且旧尝试不可能随后生效时记录可重试。
    pub async fn mark_retry_safe(&mut self) -> Result<(), RmError> {
        self.transition(BranchRecordState::RetrySafe).await
    }
    /// 在资源权威确认不可重试拒绝后持久保存拒绝结果。
    pub async fn mark_rejected(&mut self) -> Result<(), RmError> {
        self.transition(BranchRecordState::Rejected).await
    }
    async fn transition(&mut self, next: BranchRecordState) -> Result<(), RmError> {
        self.store
            .persist(self.identity.clone(), self.phase, next, self.guards.clone())
            .await?;
        self.state = Some(next);
        Ok(())
    }
}
