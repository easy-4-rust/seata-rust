use crate::my_sql_control_guard::MySqlControlGuard;
use crate::{MySqlControlMetrics, MySqlRecoveryOperation};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// 数据源签发的只读控制事务采样能力；不持有池、身份、恢复或关闭权限。
/// 对应 Java: 无直接对象；WP07 MySQL 恢复控制事务观测扩展。
/// 同数据源全部克隆与管理器共享，重新连接新数据源归零；关闭后仍可读取。
#[derive(Debug, Clone)]
pub struct MySqlRecoveryMetricsSampler {
    counts: Arc<[[AtomicU64; 7]; 6]>,
}
impl MySqlRecoveryMetricsSampler {
    /// 无参数；创建一份数据源专有的固定零值计数能力。
    pub(crate) fn new() -> Self {
        Self {
            counts: Arc::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| AtomicU64::new(0))
            })),
        }
    }
    /// 参数为固定控制动作；返回逐字段快照，不保证跨字段原子一致。
    /// 累计达到 u64 上限后饱和不回绕；active 表示当前在途调用。
    pub fn snapshot(&self, operation: MySqlRecoveryOperation) -> MySqlControlMetrics {
        let values: [u64; 7] =
            std::array::from_fn(|i| self.counts[Self::index(operation)][i].load(Ordering::Relaxed));
        MySqlControlMetrics {
            active: values[0],
            completed: values[1],
            unknown_commit: values[2],
            before_commit_error: values[3],
            cancelled: values[4],
            applied: values[5],
            unresolved: values[6],
        }
    }
    /// 参数为实际控制动作；登记一次在途调用并返回不可克隆的结算守卫。
    pub(crate) fn begin(&self, operation: MySqlRecoveryOperation) -> MySqlControlGuard {
        self.counts[Self::index(operation)][0].fetch_add(1, Ordering::Relaxed);
        MySqlControlGuard::new(self.clone(), operation)
    }
    /// 参数为实际动作和内部字段编号；无返回值，饱和增加累计。
    pub(crate) fn increment(&self, operation: MySqlRecoveryOperation, field: usize) {
        let count = &self.counts[Self::index(operation)][field];
        let mut value = count.load(Ordering::Relaxed);
        loop {
            match count.compare_exchange_weak(
                value,
                value.saturating_add(1),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => value = current,
            }
        }
    }
    /// 参数为已登记动作；无返回值，减少一份在途调用。
    pub(crate) fn leave(&self, operation: MySqlRecoveryOperation) {
        self.counts[Self::index(operation)][0].fetch_sub(1, Ordering::Relaxed);
    }
    fn index(operation: MySqlRecoveryOperation) -> usize {
        match operation {
            MySqlRecoveryOperation::Enroll => 0,
            MySqlRecoveryOperation::Observe => 1,
            MySqlRecoveryOperation::BindCommit => 2,
            MySqlRecoveryOperation::BindRollback => 3,
            MySqlRecoveryOperation::Commit => 4,
            MySqlRecoveryOperation::Rollback => 5,
        }
    }
}
#[cfg(test)]
#[path = "my_sql_recovery_metrics_sampler_tests.rs"]
mod tests;
