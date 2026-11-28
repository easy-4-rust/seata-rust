use crate::{structured_log_sampler::RECEIVED, StructuredLogSampler};
use tokio::sync::mpsc;
/// 唯一 JSON 行消费者；只有本对象能关闭接纳，采样器不能控制日志生命周期。
/// 对应 Java: 无直接对象；WP07 Rust 日志交付入口。
pub struct StructuredLogReceiver {
    pub(crate) receiver: mpsc::Receiver<Vec<u8>>,
    pub(crate) sampler: StructuredLogSampler,
}
impl StructuredLogReceiver {
    /// 无参数；异步取出一条完整 UTF-8 JSON 行，关闭并排空后返回 None；取消等待不消费记录。
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        let value = self.receiver.recv().await;
        if value.is_some() {
            self.sampler.record(RECEIVED);
        }
        value
    }
    /// 无参数；停止接纳新记录，保留已有有界队列供 recv 排空，不声称输出器已经落盘。
    pub fn close(&mut self) {
        self.receiver.close();
    }
}

impl std::fmt::Debug for StructuredLogReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructuredLogReceiver")
            .field("queued_records", &self.receiver.len())
            .finish_non_exhaustive()
    }
}
