/// 同一日志打开周期中，固定方向的保留状态与分派计数快照。
/// 对应 Java: 无直接对象；WP07 RM 持久分支观测扩展。
/// 逐字段读取不是原子总表；日志保留条目数不等于 TC 活跃分支数量。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RmBranchMetrics {
    /// 最近已确认的 Pending 保留条目数量，不推断资源未执行。
    pub journal_pending: u64,
    /// 最近已确认的 Completed 保留条目数量，不等于新资源执行次数。
    pub journal_completed: u64,
    /// 最近已确认的 RetrySafe 保留条目数量。
    pub journal_retry_safe: u64,
    /// 最近已确认的 Rejected 保留条目数量。
    pub journal_rejected: u64,
    /// 当前正在执行或等待的已轮询分派作用域数量。
    pub active: u64,
    /// 分派已返回 Applied 并确认日志同步的次数，包含恢复确认。
    pub applied: u64,
    /// 分派已返回 Rejected 并确认日志同步的次数。
    pub rejected: u64,
    /// 分派已返回 NotApplied 并确认 RetrySafe 同步的次数。
    pub retry_safe: u64,
    /// 资源明确返回 Unknown 的次数；重复观察同一分支可多次计数。
    pub unknown: u64,
    /// 资源要求人工介入的次数；保留 Pending，不伪造持久 Rejected。
    pub manual_intervention: u64,
    /// 从持久 Completed 直接复用结果的次数，不再调用资源。
    pub duplicate_completed: u64,
    /// 从持久 Rejected 直接复用结果的次数，不再调用资源。
    pub duplicate_rejected: u64,
    /// 分派返回错误次数，包括身份冲突，不推断没有资源效果。
    pub errors: u64,
    /// 未返回结果而退出的分派次数，不推断写入或资源效果边界。
    pub cancelled: u64,
    /// 进入资源 apply 调用的尝试次数，不代表业务已经生效。
    pub apply_attempts: u64,
    /// 进入资源 recover 调用的尝试次数，不代表资源已经重新执行。
    pub recover_attempts: u64,
    /// 日志追加写入或同步失败，状态 gauge 仅保留最后确认值。
    pub journal_faulted: bool,
    /// 日志所有者进入销毁，不代表操作系统文件锁已经释放。
    pub journal_closed: bool,
}
