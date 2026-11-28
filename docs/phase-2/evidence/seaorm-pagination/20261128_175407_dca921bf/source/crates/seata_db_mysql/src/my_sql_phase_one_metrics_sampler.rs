use crate::{
    my_sql_phase_one_guard::MySqlPhaseOneGuard, my_sql_phase_one_outcome::OUTCOME_COUNT,
    MySqlPhaseOneMetrics, MySqlPhaseOneOperation, MySqlPhaseOneOutcome,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// 数据源签发的一阶段只读计数能力，不拥有池、身份、提交、报告或恢复权限。
/// 对应 Java: 无直接对象；WP07 Rust 一阶段编排调用观测扩展。
#[derive(Debug, Clone)]
pub struct MySqlPhaseOneMetricsSampler {
    counts: Arc<[[AtomicU64; OUTCOME_COUNT + 1]; 10]>,
}
impl MySqlPhaseOneMetricsSampler {
    /// 无参数；创建数据源独立的零值计数能力。
    pub(crate) fn new() -> Self {
        Self {
            counts: Arc::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| AtomicU64::new(0))
            })),
        }
    }
    /// 参数为固定阶段；返回逐字段快照，不保证跨字段原子一致。
    /// 累计饱和不回绕，新数据源归零，关闭后仍可读取。
    pub fn snapshot(&self, operation: MySqlPhaseOneOperation) -> MySqlPhaseOneMetrics {
        let counts = &self.counts[operation as usize];
        MySqlPhaseOneMetrics {
            active: counts[0].load(Ordering::Relaxed),
            outcomes: std::array::from_fn(|i| counts[i + 1].load(Ordering::Relaxed)),
        }
    }
    /// 参数为实际阶段；返回已登记在途量的独占结算守卫。
    pub(crate) fn begin(&self, operation: MySqlPhaseOneOperation) -> MySqlPhaseOneGuard {
        self.counts[operation as usize][0].fetch_add(1, Ordering::Relaxed);
        MySqlPhaseOneGuard::new(self.clone(), operation)
    }
    /// 参数为阶段及结果；无返回值，饱和增加一个累计事件。
    pub(crate) fn increment(
        &self,
        operation: MySqlPhaseOneOperation,
        outcome: MySqlPhaseOneOutcome,
    ) {
        let count = &self.counts[operation as usize][outcome as usize + 1];
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
    /// 参数为已登记阶段；无返回值，减少本守卫的一份在途量。
    pub(crate) fn leave(&self, operation: MySqlPhaseOneOperation) {
        self.counts[operation as usize][0].fetch_sub(1, Ordering::Relaxed);
    }
}
#[cfg(test)]
#[path = "my_sql_phase_one_metrics_sampler_tests.rs"]
mod tests;
