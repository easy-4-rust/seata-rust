use crate::{StructuredLogFileReport, StructuredLogFileStage};
/// 文件失败只暴露固定阶段/错误类别/OS 码和确认前缀，不输出路径、原错误或日志正文。
/// 对应 Java: 无直接对象；WP07 Rust 文件输出错误边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StructuredLogFileError {
    /// 同步配置或等待预算无效。
    #[error("invalid structured log file configuration or wait budget")]
    InvalidConfiguration,
    /// 调用时没有可用 Tokio runtime。
    #[error("structured log file sink requires a Tokio runtime")]
    RuntimeRequired,
    /// 文件不是空普通文件，或原位置不为零；本 worker 未写入。
    #[error("structured log file target must be an empty regular file at position zero")]
    InvalidTarget,
    /// 下一条超出预算，当前记录未写；已有脏前缀已同步。
    #[error("structured log file byte limit reached")]
    ByteLimit {
        /// 本 worker 的已确认事实。
        progress: StructuredLogFileReport,
    },
    /// I/O 失败；确认前缀不保证未知尾部不存在，不允许自动重放。
    #[error("structured log file I/O failed at {stage:?} ({kind:?})")]
    Io {
        /// 固定调用阶段。
        stage: StructuredLogFileStage,
        /// 标准错误类别。
        kind: std::io::ErrorKind,
        /// 原 OS 数字错误码，不包含文本。
        os_error: Option<i32>,
        /// 已确认前缀。
        progress: StructuredLogFileReport,
        /// 当前原记录是否可能存在不完整尾部。
        current_record_may_be_partial: bool,
    },
    /// 异步任务取消/异常，最后观察不授权重新写入或宣称已停止。
    #[error("structured log file worker outcome is unknown")]
    WorkerOutcomeUnknown {
        /// 非原子进度下界，不是完成证明。
        last_observed: StructuredLogFileReport,
    },
}
impl StructuredLogFileError {
    /// 参数为实际 I/O、固定阶段和原确认前缀；返回脱敏错误，不记录原文本。
    pub(crate) fn io(
        source: std::io::Error,
        stage: StructuredLogFileStage,
        progress: StructuredLogFileReport,
    ) -> Self {
        Self::Io {
            stage,
            kind: source.kind(),
            os_error: source.raw_os_error(),
            progress,
            current_record_may_be_partial: matches!(
                stage,
                StructuredLogFileStage::Write | StructuredLogFileStage::Flush
            ),
        }
    }
}
