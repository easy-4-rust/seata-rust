/// 逐字段采样的进程内累计值，不保证同一瞬间一致，也不表示写入或远端接收。
/// 对应 Java: 无直接对象；WP07 Rust 日志适配器只读快照。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct StructuredLogStats {
    /// 已进入本机有界队列的条数。
    pub accepted: u64,
    /// 已交给唯一消费者的条数，不表示持久化。
    pub received: u64,
    /// 队列满而丢弃的新事件数。
    pub dropped_full: u64,
    /// 消费者关闭后丢弃的事件数。
    pub dropped_closed: u64,
    /// 格式化或编码超预算的事件数。
    pub dropped_oversize: u64,
    /// 已知 target 中模板、字段或值不合法的事件数。
    pub dropped_invalid: u64,
}
