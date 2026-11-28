use crate::StructuredLogFileError;
use std::time::Duration;
/// 显式限制单文件输出及同步节奏，不配置输出路径或提供文件创建权限。
/// 对应 Java: 无直接对象；WP07 Rust 文件输出配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuredLogFileConfig {
    pub(crate) maximum_bytes: u64,
    pub(crate) records_per_sync: u64,
    pub(crate) sync_interval: Duration,
}
impl StructuredLogFileConfig {
    /// 参数为输出上限、同步批量及周期；返回 1 GiB 内且周期 10ms–60s 的有效配置。
    pub fn new(
        maximum_bytes: u64,
        records_per_sync: u64,
        sync_interval: Duration,
    ) -> Result<Self, StructuredLogFileError> {
        if !(1..=1_073_741_824).contains(&maximum_bytes)
            || !(1..=65_536).contains(&records_per_sync)
            || !(Duration::from_millis(10)..=Duration::from_secs(60)).contains(&sync_interval)
        {
            return Err(StructuredLogFileError::InvalidConfiguration);
        }
        Ok(Self {
            maximum_bytes,
            records_per_sync,
            sync_interval,
        })
    }
}
