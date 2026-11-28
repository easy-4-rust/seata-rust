/// 资源适配器给出的权威事实；不得仅依据RM日志或缺失行猜测结果。
/// 对应 Java: ResourceManager#branchCommit/branchRollback的资源执行语义，增加显式恢复事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchResolution {
    /// 原始身份和方向的效果已持久生效；可持久记录完成。
    Applied,
    /// 确认未生效，且所有旧尝试已被围栏阻止、以后也不可能迟到生效；才允许再次调用apply。
    NotApplied,
    /// 对原始身份及方向的权威、不可重试拒绝。
    Rejected,
    /// 确认当前数据需要人工处理；停止 TC 自动重试，保留 Pending 以便原分支显式恢复。
    /// 对应 Java: BranchStatus.PhaseTwo_RollbackFailed_Unretryable 等人工处理状态。
    ManualInterventionRequired,
    /// 尚无法确定；保持Pending，只允许继续恢复查询。
    Unknown,
}
