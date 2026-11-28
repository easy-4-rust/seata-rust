use crate::StructuredLogStats;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
pub(crate) const ACCEPTED: usize = 0;
pub(crate) const RECEIVED: usize = 1;
pub(crate) const FULL: usize = 2;
pub(crate) const CLOSED: usize = 3;
pub(crate) const OVERSIZE: usize = 4;
pub(crate) const INVALID: usize = 5;
/// 克隆只保留六个原子计数，不保留队列、消费者、subscriber 或恢复权限。
/// 对应 Java: 无直接对象；WP07 Rust 日志只读采样能力。
#[derive(Debug, Clone, Default)]
pub struct StructuredLogSampler(Arc<[AtomicU64; 6]>);
impl StructuredLogSampler {
    /// 无参数；返回逐字段读取且累计饱和不回绕的快照。
    pub fn snapshot(&self) -> StructuredLogStats {
        let values = self.0.each_ref().map(|c| c.load(Ordering::Relaxed));
        StructuredLogStats {
            accepted: values[ACCEPTED],
            received: values[RECEIVED],
            dropped_full: values[FULL],
            dropped_closed: values[CLOSED],
            dropped_oversize: values[OVERSIZE],
            dropped_invalid: values[INVALID],
        }
    }
    /// 参数为内部固定计数索引；只递增计数，不执行日志、I/O 或用户回调。
    pub(crate) fn record(&self, index: usize) {
        if let Some(counter) = self.0.get(index) {
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
    }
}

#[cfg(test)]
mod tests {
    use super::StructuredLogSampler;
    use std::sync::atomic::Ordering;
    #[test]
    fn counters_saturate_and_clones_share_only_counters() {
        let sampler = StructuredLogSampler::default();
        let clone = sampler.clone();
        for index in 0..6 {
            sampler.0[index].store(u64::MAX - 1, Ordering::Relaxed);
            clone.record(index);
            clone.record(index);
            assert_eq!(sampler.0[index].load(Ordering::Relaxed), u64::MAX);
        }
        assert_eq!(clone.snapshot().accepted, u64::MAX);
    }
}
