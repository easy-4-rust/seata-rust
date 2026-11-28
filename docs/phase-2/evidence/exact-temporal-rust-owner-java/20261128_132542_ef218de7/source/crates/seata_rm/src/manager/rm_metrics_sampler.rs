use super::rm_dispatch_guard::RmDispatchGuard;
use crate::{BranchPhase, BranchRecordState, RmBranchMetrics};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

/// 只读 RM 分支采样器；只持有固定计数，不持有文件、租约或资源分派能力。
/// 对应 Java: 无直接对象；WP07 RM 日志与分派观测扩展。
/// 所有共享日志的分派器共用计数；重开后状态重建而分派累计从零开始。
#[derive(Debug, Clone)]
pub struct RmMetricsSampler {
    counts: Arc<[[AtomicU64; 16]; 2]>,
    lifecycle: Arc<[AtomicBool; 2]>,
}

impl RmMetricsSampler {
    /// 无参数；返回零值固定容量计数源，不授予公开的写入能力。
    pub(crate) fn new() -> Self {
        Self {
            counts: Arc::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| AtomicU64::new(0))
            })),
            lifecycle: Arc::new([AtomicBool::new(false), AtomicBool::new(false)]),
        }
    }

    /// 参数为原方向；返回逐字段读取的状态与事件快照，不保证跨字段同一时刻。
    /// gauge 是日志保留条目，counter 是本次打开周期的事件；计数不包含身份或路径。
    pub fn snapshot(&self, phase: BranchPhase) -> RmBranchMetrics {
        let values: [u64; 16] =
            std::array::from_fn(|i| self.counts[Self::phase(phase)][i].load(Ordering::Relaxed));
        RmBranchMetrics {
            journal_pending: values[0],
            journal_completed: values[1],
            journal_retry_safe: values[2],
            journal_rejected: values[3],
            active: values[4],
            applied: values[5],
            rejected: values[6],
            retry_safe: values[7],
            unknown: values[8],
            manual_intervention: values[9],
            duplicate_completed: values[10],
            duplicate_rejected: values[11],
            errors: values[12],
            cancelled: values[13],
            apply_attempts: values[14],
            recover_attempts: values[15],
            journal_faulted: self.lifecycle[0].load(Ordering::Relaxed),
            journal_closed: self.lifecycle[1].load(Ordering::Relaxed),
        }
    }

    /// 参数为经同一日志校验的方向、旧状态和新状态；无返回值，维护已确认索引 gauge。
    pub(crate) fn transition(
        &self,
        phase: BranchPhase,
        previous: Option<BranchRecordState>,
        next: BranchRecordState,
    ) {
        if previous == Some(next) {
            return;
        }
        let counts = &self.counts[Self::phase(phase)];
        if let Some(previous) = previous {
            counts[Self::state(previous)].fetch_sub(1, Ordering::Relaxed);
        }
        counts[Self::state(next)].fetch_add(1, Ordering::Relaxed);
    }

    /// 参数为方向；增加 active 并返回独占结算守卫。
    pub(crate) fn begin(&self, phase: BranchPhase) -> RmDispatchGuard {
        self.counts[Self::phase(phase)][4].fetch_add(1, Ordering::Relaxed);
        RmDispatchGuard::new(self.clone(), phase)
    }

    /// 参数为方向及固定内部字段；无返回值，以兼容 MSRV 的 CAS 饱和增加累计值。
    pub(crate) fn increment(&self, phase: BranchPhase, field: usize) {
        let counter = &self.counts[Self::phase(phase)][field];
        let mut value = counter.load(Ordering::Relaxed);
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

    /// 参数为方向；无返回值，归还一份已登记的 active。
    pub(crate) fn leave(&self, phase: BranchPhase) {
        self.counts[Self::phase(phase)][4].fetch_sub(1, Ordering::Relaxed);
    }
    /// 无参数、无返回值；记录追加失败，保留最后确认的索引 gauge。
    pub(crate) fn faulted(&self) {
        self.lifecycle[0].store(true, Ordering::Relaxed);
    }
    /// 无参数、无返回值；记录日志所有者进入销毁。
    pub(crate) fn closed(&self) {
        self.lifecycle[1].store(true, Ordering::Relaxed);
    }

    fn phase(phase: BranchPhase) -> usize {
        match phase {
            BranchPhase::Commit => 0,
            BranchPhase::Rollback => 1,
        }
    }
    fn state(state: BranchRecordState) -> usize {
        match state {
            BranchRecordState::Pending => 0,
            BranchRecordState::Completed => 1,
            BranchRecordState::RetrySafe => 2,
            BranchRecordState::Rejected => 3,
        }
    }
}

#[cfg(test)]
#[path = "rm_metrics_sampler_tests.rs"]
mod tests;
