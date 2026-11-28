use crate::{
    log_file_progress::LogFileProgress, StructuredLogFileConfig, StructuredLogFileError,
    StructuredLogFileReport, StructuredLogFileShutdown, StructuredLogReceiver,
};
use std::{sync::Arc, time::Duration};
use tokio::{sync::oneshot, task::JoinHandle};
/// 显式拥有唯一文件任务的控制器；超时不丢弃原 JoinHandle，不自动创建路径或重放。
/// 对应 Java: 无直接对象；WP07 Rust 有界单文件输出器。
pub struct StructuredLogFileSink {
    worker: Option<JoinHandle<Result<StructuredLogFileReport, StructuredLogFileError>>>,
    stop: Option<oneshot::Sender<()>>,
    progress: Arc<LogFileProgress>,
    terminal: Option<Result<StructuredLogFileReport, StructuredLogFileError>>,
}
impl StructuredLogFileSink {
    /// 参数为唯一消费者、调用方独占的空普通文件及预算；返回已启动的异步输出器。
    /// 不创建路径或截断文件；无 runtime 返回错误，目标校验及 I/O 错误由 wait/shutdown 取得。
    pub fn start(
        receiver: StructuredLogReceiver,
        file: std::fs::File,
        config: StructuredLogFileConfig,
    ) -> Result<Self, StructuredLogFileError> {
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|_| StructuredLogFileError::RuntimeRequired)?;
        let (tx, rx) = oneshot::channel();
        let progress = Arc::new(LogFileProgress::default());
        let worker = handle.spawn(crate::log_file_worker::run(
            receiver,
            file,
            config,
            rx,
            progress.clone(),
        ));
        Ok(Self {
            worker: Some(worker),
            stop: Some(tx),
            progress,
            terminal: None,
        })
    }
    /// 无参数；请求原任务停止接纳并排空，不将请求发出等同于已停止或已同步。
    pub fn request_shutdown(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
    /// 无参数；返回只读确认前缀快照，不证明无在途 I/O。
    pub fn progress(&self) -> StructuredLogFileReport {
        self.progress.snapshot()
    }
    /// 无参数；等待原任务的成功或错误，可取消等待而保留句柄；重复观察使用缓存结果。
    pub async fn wait(&mut self) -> Result<StructuredLogFileReport, StructuredLogFileError> {
        if let Some(result) = self.terminal {
            return result;
        }
        let result = match self.worker.as_mut() {
            Some(worker) => worker.await.unwrap_or_else(|_| {
                Err(StructuredLogFileError::WorkerOutcomeUnknown {
                    last_observed: self.progress.snapshot(),
                })
            }),
            None => Err(StructuredLogFileError::WorkerOutcomeUnknown {
                last_observed: self.progress.snapshot(),
            }),
        };
        self.worker.take();
        self.terminal = Some(result);
        result
    }
    /// 参数为 0–60s 等待预算；超时返回 Pending 并保留同一任务，完成后返回最终同步报告。
    pub async fn shutdown(
        &mut self,
        budget: Duration,
    ) -> Result<StructuredLogFileShutdown, StructuredLogFileError> {
        if budget > Duration::from_secs(60) {
            return Err(StructuredLogFileError::InvalidConfiguration);
        }
        self.request_shutdown();
        match tokio::time::timeout(budget, self.wait()).await {
            Ok(result) => result.map(StructuredLogFileShutdown::Complete),
            Err(_) => Ok(StructuredLogFileShutdown::Pending),
        }
    }
}
impl Drop for StructuredLogFileSink {
    fn drop(&mut self) {
        self.request_shutdown();
    }
}
impl std::fmt::Debug for StructuredLogFileSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructuredLogFileSink")
            .field("progress", &self.progress.snapshot())
            .field("terminal", &self.terminal)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "structured_log_file_sink_tests.rs"]
mod tests;
