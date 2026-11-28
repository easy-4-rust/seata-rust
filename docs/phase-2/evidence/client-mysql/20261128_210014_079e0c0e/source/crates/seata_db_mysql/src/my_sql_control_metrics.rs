/// 同一数据源克隆共享的固定控制动作计数快照；不是数据库持久状态 gauge。
/// 对应 Java: 无直接对象；WP07 MySQL 恢复控制事务观测扩展。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MySqlControlMetrics {
    /// 当前正在等待结果的控制调用数量。
    pub active: u64,
    /// 控制事务在期限内返回 Ok；不等于业务成功或本次执行了业务效果。
    pub completed: u64,
    /// 原错误明确属于控制 COMMIT 确认未知的数量。
    pub unknown_commit: u64,
    /// 本次控制 COMMIT 前失败；不能推断旧尝试或业务没有效果。
    pub before_commit_error: u64,
    /// Future 被取消或 panic 未返回结果的数量；不推断提交边界。
    pub cancelled: u64,
    /// Ok(Applied) 数量，包含读取已存在终态，不能作为效果执行次数。
    pub applied: u64,
    /// Ok(Unknown) 数量，包含登记、方向绑定及无确定终态的控制观察。
    pub unresolved: u64,
}
