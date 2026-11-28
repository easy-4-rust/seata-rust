use super::CommonFenceStatus;
/// 仅供同一持锁事务内执行的 Fence 计划，不证明提交成功或原 TC 授权。
/// 对应 Java: org.apache.seata.rm.fence.SpringFenceHandler 的条件分支；Rust 显式计划扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceDecision {
    /// 唯一插入该状态后，必须在同事务调用 Try；插入失败不能调用业务。
    InsertAndInvoke(CommonFenceStatus),
    /// 比较原状态更新，成功才在同事务调用业务；业务失败则全部回滚。
    UpdateAndInvoke {
        /// 比较更新必须匹配的原 TRIED 状态。
        expected: CommonFenceStatus,
        /// 与业务同事务写入的原方向终态。
        next: CommonFenceStatus,
    },
    /// 空 Cancel 唯一插入 SUSPENDED 并确认提交后才能返回完成，不执行业务。
    InsertSuspended,
    /// 同方向已完成，不重复业务；仍须由存储验证原身份和持久事实。
    AlreadyApplied,
    /// 已存在 Try 或原方向冲突；不得执行业务、覆盖或删除原记录。
    Refuse,
    /// Confirm 无 Try 记录，返回失败；缺行不证明 Confirm 完成。
    MissingTry,
}
