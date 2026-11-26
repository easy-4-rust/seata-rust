use crate::RmError;
use tokio::sync::Semaphore;

/// 日志磁盘、记录数及并发租约的显式预算；没有自动驱逐或自动修复。
/// 对应 Java: RM资源管理边界；Rust持久协调配置扩展。
#[derive(Debug, Clone, Copy)]
pub struct JournalOptions {
    pub(crate) max_branches: usize,
    pub(crate) max_file_bytes: u64,
    pub(crate) max_record_bytes: usize,
    pub(crate) max_leases: usize,
}
impl JournalOptions {
    /// 参数依次为分支数、总文件字节、单条记录总字节及租约数；返回已校验预算。
    pub fn new(
        max_branches: usize,
        max_file_bytes: u64,
        max_record_bytes: usize,
        max_leases: usize,
    ) -> Result<Self, RmError> {
        if max_branches == 0
            || max_leases == 0
            || max_leases > Semaphore::MAX_PERMITS
            || !(32..=2_097_152).contains(&max_record_bytes)
            || max_file_bytes < max_record_bytes as u64 + 8
        {
            return Err(RmError::Invalid {
                reason:
                    "nonzero bounded leases/branches and sufficient file/record budgets required",
            });
        }
        Ok(Self {
            max_branches,
            max_file_bytes,
            max_record_bytes,
            max_leases,
        })
    }
}
