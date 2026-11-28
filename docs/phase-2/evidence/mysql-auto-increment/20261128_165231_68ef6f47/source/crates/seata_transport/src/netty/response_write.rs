use super::write_receipt::WriteReceipt;
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;
/// 排队/执行中的原代际响应写任务，独占预算；worker或队列释放仍必须结算回执。
/// 对应 Java: ChannelOutboundBuffer的写任务/ChannelPromise所有权。
pub(crate) struct ResponseWrite {
    pub(crate) bytes: Vec<u8>,
    pub(crate) receipt: Arc<WriteReceipt>,
    pub(crate) _permit: OwnedSemaphorePermit,
}
impl Drop for ResponseWrite {
    fn drop(&mut self) {
        self.receipt.fail(self.receipt.close_reason());
    }
}
