/// 原始二阶段方向；同一分支记录不能切换方向。
/// 对应 Java: BranchCommitRequest / BranchRollbackRequest分派。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchPhase {
    /// TC提交指令。
    Commit,
    /// TC回滚指令。
    Rollback,
}
