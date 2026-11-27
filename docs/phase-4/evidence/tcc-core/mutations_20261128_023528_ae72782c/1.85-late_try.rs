use super::{CommonFenceStatus, FenceDecision, FenceOperation};
/// 分离原 Java 三阶段 Fence 决策与数据库副作用。
/// 对应 Java: org.apache.seata.rm.fence.SpringFenceHandler#prepareFence/commitFence/rollbackFence。
#[derive(Debug)]
pub struct FenceHandler;
impl FenceHandler {
    /// 参数为当前操作和持锁事务中读取的状态；返回仍需原子执行和提交的计划。
    /// 缺失行不得脱离唯一键插入仲裁直接声明空回滚成功。
    pub const fn plan(
        operation: FenceOperation,
        existing: Option<CommonFenceStatus>,
    ) -> FenceDecision {
        use CommonFenceStatus::{Committed, Rollbacked, Suspended, Tried};
        match (operation, existing) {
            (FenceOperation::Try, None) => FenceDecision::InsertAndInvoke(Tried),
            (FenceOperation::Try, Some(_)) => FenceDecision::InsertAndInvoke(Tried),
            (FenceOperation::Confirm, None) => FenceDecision::MissingTry,
            (FenceOperation::Confirm, Some(Tried)) => FenceDecision::UpdateAndInvoke {
                expected: Tried,
                next: Committed,
            },
            (FenceOperation::Confirm, Some(Committed)) => FenceDecision::AlreadyApplied,
            (FenceOperation::Confirm, Some(Rollbacked | Suspended)) => FenceDecision::Refuse,
            (FenceOperation::Cancel, None) => FenceDecision::InsertSuspended,
            (FenceOperation::Cancel, Some(Tried)) => FenceDecision::UpdateAndInvoke {
                expected: Tried,
                next: Rollbacked,
            },
            (FenceOperation::Cancel, Some(Rollbacked | Suspended)) => FenceDecision::AlreadyApplied,
            (FenceOperation::Cancel, Some(Committed)) => FenceDecision::Refuse,
        }
    }
}
