/// 原提交尝试经过持久见证或 Undo 唯一键围栏后的可确认结果。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 的跨进程恢复扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MySqlCommitRecoveryOutcome {
    /// 原 004 见证与身份及未终结 Normal Undo 一致，业务一阶段已持久提交。
    Applied,
    /// 原 Undo 唯一键 guard 已持久确认，重新核对 004 见证仍为零。
    /// 该结果不授权重放业务 SQL、重新向 TC 登记或推断全局事务终态。
    NotAppliedFenced,
}
