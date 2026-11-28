use super::tm_call_guard::TmCallGuard;
use crate::{TmOperation, TmOperationMetrics};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// 仅允许读取 TM 固定操作指标的能力；不授予请求、恢复、关闭或计数修改权限。
/// 对应 Java: 无直接对象；WP07 只读 TM 指标采样扩展。
/// 管理器关闭后仍保留最终累计值；不跨进程持久化或关联业务身份。
#[derive(Debug, Clone)]
pub struct TmMetricsSampler {
    counts: Arc<[[AtomicU64; 8]; 4]>,
}

impl TmMetricsSampler {
    /// 无参数；创建当前管理器专有的零值固定容量计数源。
    pub(crate) fn new() -> Self {
        Self {
            counts: Arc::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| AtomicU64::new(0))
            })),
        }
    }

    /// 参数为固定 TM 操作；返回逐字段读取的当前快照，不保证跨字段同一时刻。
    /// 累计计数达到 u64 上限时饱和，不回绕；active 是当前在途数量。
    pub fn snapshot(&self, operation: TmOperation) -> TmOperationMetrics {
        let fields = &self.counts[Self::index(operation)];
        let values: [u64; 8] = std::array::from_fn(|index| fields[index].load(Ordering::Relaxed));
        TmOperationMetrics {
            active: values[0],
            observed: values[1],
            tc_rejected: values[2],
            unknown: values[3],
            not_sent: values[4],
            local_rejected: values[5],
            cancelled: values[6],
            protocol_errors: values[7],
        }
    }

    /// 参数为固定操作；增加当前调用数量并返回独占结算守卫。
    pub(crate) fn begin(&self, operation: TmOperation) -> TmCallGuard {
        self.counts[Self::index(operation)][0].fetch_add(1, Ordering::Relaxed);
        TmCallGuard::new(self.clone(), operation)
    }

    /// 参数为固定操作及内部字段索引；无返回值，以饱和方式增加累计计数。
    pub(crate) fn increment(&self, operation: TmOperation, field: usize) {
        let counter = &self.counts[Self::index(operation)][field];
        let mut value = counter.load(Ordering::Relaxed);
        // 用兼容 MSRV 的 CAS 实现饱和增量，不回绕为零。
        loop {
            match counter.compare_exchange_weak(
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

    /// 参数为原固定操作；无返回值，归还一份已登记的当前调用数量。
    pub(crate) fn leave(&self, operation: TmOperation) {
        self.counts[Self::index(operation)][0].fetch_sub(1, Ordering::Relaxed);
    }

    fn index(operation: TmOperation) -> usize {
        match operation {
            TmOperation::Begin => 0,
            TmOperation::Commit => 1,
            TmOperation::Rollback => 2,
            TmOperation::Status => 3,
        }
    }
}

#[cfg(test)]
#[path = "tm_metrics_sampler_tests.rs"]
mod tests;
