use crate::StructuredLogError;
/// 显式限制日志队列与单条编码大小；预算不包括应用自己的写入器或并发线程。
/// 对应 Java: 无直接对象；WP07 Rust 结构化日志订阅配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuredLogConfig {
    pub(crate) capacity: usize,
    pub(crate) maximum_record_bytes: usize,
}
impl StructuredLogConfig {
    /// 参数为队列条数及含换行的 JSON 字节上限；返回总内容预算不超过 64 MiB 的配置。
    pub fn new(capacity: usize, maximum_record_bytes: usize) -> Result<Self, StructuredLogError> {
        if !(1..=4096).contains(&capacity)
            || !(512..=262_144).contains(&maximum_record_bytes)
            || capacity
                .checked_mul(maximum_record_bytes)
                .is_none_or(|bytes| bytes > 64 * 1024 * 1024)
        {
            return Err(StructuredLogError::InvalidConfiguration);
        }
        Ok(Self {
            capacity,
            maximum_record_bytes,
        })
    }
}
