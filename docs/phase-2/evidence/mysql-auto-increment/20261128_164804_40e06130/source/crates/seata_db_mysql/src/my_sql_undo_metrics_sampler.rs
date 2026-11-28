use crate::{my_sql_undo_guard::MySqlUndoGuard, MySqlUndoMetrics, MySqlUndoOperation};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// 数据源签发的只读 Undo 调用采样能力；不持有连接池、身份或恢复权限。
/// 对应 Java: 无直接对象；WP07 独立 MySQL Undo 调用观测扩展。
#[derive(Debug, Clone)]
pub struct MySqlUndoMetricsSampler {
    counts: Arc<[[AtomicU64; 10]; 2]>,
}
impl MySqlUndoMetricsSampler {
    /// 无参数；创建独立数据源的固定零值计数能力。
    pub(crate) fn new() -> Self {
        Self {
            counts: Arc::new(std::array::from_fn(|_| {
                std::array::from_fn(|_| AtomicU64::new(0))
            })),
        }
    }
    /// 参数为固定调用类别；返回逐字段快照，不保证跨字段原子一致。
    /// 累计达到 u64 上限后饱和，进程重启或新数据源从零开始，关闭后仍可读。
    pub fn snapshot(&self, operation: MySqlUndoOperation) -> MySqlUndoMetrics {
        let v: [u64; 10] =
            std::array::from_fn(|i| self.counts[Self::index(operation)][i].load(Ordering::Relaxed));
        MySqlUndoMetrics {
            active: v[0],
            completed: v[1],
            unknown_commit: v[2],
            before_commit_error: v[3],
            cancelled: v[4],
            consumed: v[5],
            fenced_inserted: v[6],
            fenced_existing: v[7],
            undo_present: v[8],
            observed_restored_rows: v[9],
        }
    }
    /// 参数为实际调用类别；登记在途调用并返回独占结算守卫。
    pub(crate) fn begin(&self, operation: MySqlUndoOperation) -> MySqlUndoGuard {
        self.counts[Self::index(operation)][0].fetch_add(1, Ordering::Relaxed);
        MySqlUndoGuard::new(self.clone(), operation)
    }
    /// 参数为动作、内部字段与增量；无返回值，累计饱和不回绕。
    pub(crate) fn add(&self, operation: MySqlUndoOperation, field: usize, amount: u64) {
        let count = &self.counts[Self::index(operation)][field];
        let mut value = count.load(Ordering::Relaxed);
        loop {
            match count.compare_exchange_weak(
                value,
                value.saturating_add(amount),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => value = current,
            }
        }
    }
    /// 参数为已登记调用类别；无返回值，减少本守卫持有的一份在途量。
    pub(crate) fn leave(&self, operation: MySqlUndoOperation) {
        self.counts[Self::index(operation)][0].fetch_sub(1, Ordering::Relaxed);
    }
    fn index(operation: MySqlUndoOperation) -> usize {
        match operation {
            MySqlUndoOperation::Rollback => 0,
            MySqlUndoOperation::Fence => 1,
        }
    }
}
