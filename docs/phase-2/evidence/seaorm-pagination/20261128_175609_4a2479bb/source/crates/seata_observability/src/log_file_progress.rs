use crate::StructuredLogFileReport;
use std::sync::atomic::{AtomicU64, Ordering};
/// 只存五个进度计数，不保留文件、队列或发送/关闭权限。
/// 对应 Java: 无直接对象；WP07 Rust 文件输出进度采样。
#[derive(Debug, Default)]
pub(crate) struct LogFileProgress {
    received: AtomicU64,
    written: AtomicU64,
    bytes: AtomicU64,
    synchronized: AtomicU64,
    sync_operations: AtomicU64,
}
impl LogFileProgress {
    /// 参数为 worker 的确认前缀；无返回值，在同步记录发布前更新其余计数。
    pub(crate) fn publish(&self, r: StructuredLogFileReport) {
        self.received.store(r.received_records, Ordering::Relaxed);
        self.written.store(r.written_records, Ordering::Relaxed);
        self.bytes.store(r.written_bytes, Ordering::Relaxed);
        self.sync_operations
            .store(r.sync_operations, Ordering::Relaxed);
        self.synchronized
            .store(r.synchronized_records, Ordering::Release);
    }
    /// 无参数；返回逐字段快照，不声称同一瞬间一致或无在途 I/O。
    pub(crate) fn snapshot(&self) -> StructuredLogFileReport {
        let synchronized = self.synchronized.load(Ordering::Acquire);
        StructuredLogFileReport {
            received_records: self.received.load(Ordering::Relaxed),
            written_records: self.written.load(Ordering::Relaxed),
            written_bytes: self.bytes.load(Ordering::Relaxed),
            synchronized_records: synchronized,
            sync_operations: self.sync_operations.load(Ordering::Relaxed),
        }
    }
}
