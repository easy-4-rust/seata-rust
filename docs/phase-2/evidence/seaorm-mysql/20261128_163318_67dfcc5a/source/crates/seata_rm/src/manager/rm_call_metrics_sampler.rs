use super::rm_call_guard::RmCallGuard;
use crate::{RmOperation, RmOperationMetrics};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// 仅允许读取 RM 固定操作指标的能力；不授予请求、恢复、关闭或计数修改权限。
/// 对应 Java: 无直接对象；WP07 只读 RM 指标采样扩展。
/// 管理器关闭后仍可读取；保留客户端的本地调用仍可增加累计，不跨进程持久化或关联业务身份。
#[derive(Debug, Clone)]
pub struct RmCallMetricsSampler {
    counts: Arc<[[AtomicU64; 8]; 3]>,
}

impl RmCallMetricsSampler {
    /// 无参数；创建当前管理器专有的零值固定容量计数源。
    pub(crate) fn new() -> Self {
        Self {
            counts: Arc::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| AtomicU64::new(0))
            })),
        }
    }

    /// 参数为固定 RM 操作；返回逐字段读取的当前快照，不保证跨字段同一时刻。
    /// 累计计数达到 u64 上限时饱和，不回绕；active 是当前在途数量。
    pub fn snapshot(&self, operation: RmOperation) -> RmOperationMetrics {
        let fields = &self.counts[Self::index(operation)];
        let values: [u64; 8] = std::array::from_fn(|index| fields[index].load(Ordering::Relaxed));
        RmOperationMetrics {
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
    pub(crate) fn begin(&self, operation: RmOperation) -> RmCallGuard {
        self.counts[Self::index(operation)][0].fetch_add(1, Ordering::Relaxed);
        RmCallGuard::new(self.clone(), operation)
    }

    /// 参数为固定操作及内部字段索引；无返回值，以饱和方式增加累计计数。
    pub(crate) fn increment(&self, operation: RmOperation, field: usize) {
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
    pub(crate) fn leave(&self, operation: RmOperation) {
        self.counts[Self::index(operation)][0].fetch_sub(1, Ordering::Relaxed);
    }

    fn index(operation: RmOperation) -> usize {
        match operation {
            RmOperation::BranchRegister => 0,
            RmOperation::BranchReport => 1,
            RmOperation::LockQuery => 2,
        }
    }
}

#[cfg(test)]
#[path = "rm_call_metrics_sampler_tests.rs"]
mod tests;
