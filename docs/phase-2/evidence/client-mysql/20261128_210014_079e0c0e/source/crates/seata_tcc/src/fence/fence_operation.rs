/// 业务三阶段操作，不与全局事务状态或 RM 分支状态混用。
/// 对应 Java: org.apache.seata.integration.tx.api.fence.FenceHandler 的三个执行方法；Rust 显式操作扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceOperation {
    /// 原 prepareFence / Try。
    Try,
    /// 原 commitFence / Confirm。
    Confirm,
    /// 原 rollbackFence / Cancel。
    Cancel,
}
