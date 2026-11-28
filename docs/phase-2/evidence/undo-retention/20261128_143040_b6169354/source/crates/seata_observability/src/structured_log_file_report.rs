/// 本 worker 的文件确认前缀；进度采样不是停止或无在途写入证明。
/// 对应 Java: 无直接对象；WP07 Rust 文件输出事实。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct StructuredLogFileReport {
    /// 本 worker 已取出的原记录数，包含最终未写入的记录。
    pub received_records: u64,
    /// write_all 和 flush 均明确成功的完整记录数。
    pub written_records: u64,
    /// 完整写入记录的字节数，不计未确认尾部。
    pub written_bytes: u64,
    /// 最近成功 sync_all 已覆盖的完整记录数。
    pub synchronized_records: u64,
    /// 成功 sync_all 调用次数，不表示事务效果。
    pub sync_operations: u64,
}
