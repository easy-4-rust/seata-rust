use crate::StructuredLogFileReport;
/// 关闭等待的明确结果，Pending 不取消原任务或授予重启权限。
/// 对应 Java: 无直接对象；WP07 Rust 文件输出关闭结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredLogFileShutdown {
    /// 原任务已排空、最终同步并返回确认报告。
    Complete(StructuredLogFileReport),
    /// 原 JoinHandle 仍由 sink 保留，可继续等待。
    Pending,
}
