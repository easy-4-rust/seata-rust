/// 同一 TM 管理器和固定操作的进程内调用计数快照。
/// 对应 Java: 无直接对象；WP07 TM 调用结果观测扩展。
/// 逐字段采样不是全局原子视图；observed 不表示全局事务已最终完成。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TmOperationMetrics {
    /// 当前尚未返回或取消的调用数量。
    pub active: u64,
    /// 已收到且通过 TM 语义校验的权威成功响应数量，状态可以尚未完成。
    pub observed: u64,
    /// TC 明确 Failed 响应数量，不推断 TC 没有副作用。
    pub tc_rejected: u64,
    /// 原错误分类认为可能已生效的数量，包含写后协议错误。
    pub unknown: u64,
    /// 有明确写前证据的调用失败数量。
    pub not_sent: u64,
    /// 进入统计范围后被本地前提校验拒绝的数量。
    pub local_rejected: u64,
    /// Future 未返回结果而被取消或 panic 退出的数量，不能推断写入边界。
    pub cancelled: u64,
    /// 协议错误附加计数；与 unknown 或 not_sent 分类可以重叠。
    pub protocol_errors: u64,
}
