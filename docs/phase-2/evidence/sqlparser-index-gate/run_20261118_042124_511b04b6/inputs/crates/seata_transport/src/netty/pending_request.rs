use super::response_expectation::ResponseExpectation;
use crate::{CallError, ConnectionGeneration, TransportError};
use seata_protocol::protocol::RpcMessage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, OwnedSemaphorePermit};
use tokio::time::Instant;

/// 单个等待项的完成能力、期限和写入标记，帧字节由有界写队列持有。
/// 对应 Java: org.apache.seata.core.protocol.MessageFuture 的等待元数据；Rust使用独占oneshot完成权。
pub(crate) struct PendingRequest {
    pub(crate) expected: ResponseExpectation,
    pub(crate) deadline: Instant,
    pub(crate) reply: oneshot::Sender<Result<RpcMessage, CallError>>,
    pub(crate) attempted: Arc<AtomicBool>,
    pub(crate) _permit: OwnedSemaphorePermit,
}
impl PendingRequest {
    /// 消费此完成能力，按原子写入标记发送唯一错误；receiver已取消时丢弃返回值。
    pub(crate) fn fail(self, generation: ConnectionGeneration, id: i32, reason: TransportError) {
        let error = CallError::request(
            generation,
            id,
            self.attempted.load(Ordering::Acquire),
            reason,
        );
        let _ = self.reply.send(Err(error));
    }
}
