use seata_rm::{BranchCallbackReceipt, BranchPhase, RegisteredBranch};

/// 独立恢复事务的封闭动作集合；登记和方向仲裁不能与补偿终态拆错事务。
/// 对应 Java: 无直接对象；WP05 Rust 持久恢复扩展的内部调用对象。
pub(crate) enum RecoveryAction<'a> {
    Enroll(&'a RegisteredBranch),
    Observe(&'a BranchCallbackReceipt),
    Bind(BranchPhase),
    Rollback,
    Commit,
}
