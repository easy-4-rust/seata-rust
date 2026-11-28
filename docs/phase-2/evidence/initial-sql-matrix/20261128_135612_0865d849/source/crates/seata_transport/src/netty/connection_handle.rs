use super::outbound_command::OutboundCommand;
use super::pending_request::PendingRequest;
use super::pending_request_guard::PendingRequestGuard;
use super::pending_requests::PendingRequests;
use super::response_expectation::ResponseExpectation;
use super::response_write::ResponseWrite;
use super::write_receipt::WriteReceipt;
use crate::{CallError, ConnectionConfig, ConnectionGeneration, ConnectionStats, TransportError};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::v1::ProtocolEncoderV1;
use seata_protocol::{CompressorType, Message, RequestType, SerializerType};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::time::Instant;

/// 可克隆的单会话调用入口；克隆共享同一有界等待表，不复制ID分配器或重放请求。
/// 对应 Java: AbstractNettyRemotingClient#sendSync 的单通道调用职责。
#[derive(Clone)]
pub struct ConnectionHandle {
    pub(crate) core: Arc<PendingRequests>,
    pub(crate) outbound: mpsc::Sender<OutboundCommand>,
    pub(crate) permits: Arc<Semaphore>,
    pub(crate) response_permits: Arc<Semaphore>,
    pub(crate) config: ConnectionConfig,
}
impl fmt::Debug for ConnectionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionHandle")
            .field("stats", &self.snapshot())
            .finish()
    }
}
impl ConnectionHandle {
    /// 返回此句柄永久绑定的连接代际，旧句柄不会迁移到重连后的通道。
    pub fn generation(&self) -> ConnectionGeneration {
        self.core.generation()
    }
    /// 返回一致的单连接元数据快照，不展示请求或头内容。
    pub fn snapshot(&self) -> ConnectionStats {
        self.core.snapshot()
    }
    /// 主动关闭本句柄绑定的物理代际并按各写入标记清理等待；不关闭整个池、不迁移或重发调用。
    /// 对应 Java: io.netty.channel.Channel#close。
    pub fn close(&self) {
        self.core.terminate(TransportError::ConnectionClosed);
    }
    /// 等待此代际永久终止并返回首次原因；可取消，不迁移到替换连接或丢失关闭通知。
    pub async fn closed(&self) -> TransportError {
        let mut closed = self.core.subscribe_closed();
        loop {
            if *closed.borrow_and_update() {
                return self
                    .snapshot()
                    .last_failure
                    .unwrap_or(TransportError::ConnectionClosed);
            }
            if closed.changed().await.is_err() {
                return TransportError::ConnectionClosed;
            }
        }
    }
    /// 仅供原代际入站能力调用：参数是原信封、已验证业务回包及期限，成功只表示完整本地写入。
    pub(crate) async fn respond(
        &self,
        request: &RpcMessage,
        body: Message,
        timeout: Duration,
    ) -> Result<(), CallError> {
        let local = |reason| CallError::local(Some(self.generation()), Some(request.id), reason);
        let deadline = Instant::now()
            .checked_add(timeout)
            .filter(|_| !timeout.is_zero())
            .ok_or_else(|| local(TransportError::ResponseTimeout))?;
        if self.snapshot().closed {
            return Err(local(TransportError::ConnectionClosed));
        }
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(local(TransportError::InvalidConfiguration {
                reason: "Tokio I/O and time runtime required",
            }));
        }
        if request.message_type != RequestType::RequestSync {
            return Err(local(TransportError::UnexpectedResponse {
                reason: "only synchronous request envelopes can receive ordinary replies",
            }));
        }
        let original = request.body.as_ref().ok_or_else(|| {
            local(TransportError::UnexpectedResponse {
                reason: "inbound reply requires request payload",
            })
        })?;
        let expected =
            ResponseExpectation::from_request(original, self.config.frames.payload_limits())
                .map_err(local)?;
        if !expected.matches(&body) || !ResponseExpectation::reply_identity_matches(original, &body)
        {
            return Err(local(TransportError::UnexpectedResponse {
                reason: "reply family, batch shape or branch identity differs",
            }));
        }
        let permit = Arc::clone(&self.response_permits)
            .try_acquire_owned()
            .map_err(|_| local(TransportError::Overloaded))?;
        let queue = self.outbound.try_reserve().map_err(|error| {
            local(match error {
                mpsc::error::TrySendError::Full(_) => TransportError::Overloaded,
                mpsc::error::TrySendError::Closed(_) => TransportError::ConnectionClosed,
            })
        })?;
        let frame = RpcMessage {
            id: request.id,
            message_type: RequestType::Response,
            codec: SerializerType::Seata,
            compressor: CompressorType::None,
            head_map: request.head_map.clone(),
            body: Some(body),
        };
        let bytes = ProtocolEncoderV1::new(self.config.frames)
            .encode(&frame)
            .map_err(|reason| local(reason.into()))?;
        let (sender, mut receiver) = oneshot::channel();
        let receipt = Arc::new(WriteReceipt::new(
            Arc::clone(&self.core),
            request.id,
            deadline,
            sender,
        ));
        queue.send(OutboundCommand::Response(ResponseWrite {
            bytes,
            receipt: Arc::clone(&receipt),
            _permit: permit,
        }));
        tokio::select! {biased;
            result=&mut receiver=>match result {Ok(result)=>result,Err(_)=>Err(receipt.lost_completion())},
            reason=self.closed()=>{if let Some(error)=receipt.fail(reason) {Err(error)} else {match receiver.try_recv() {Ok(result)=>result,Err(_)=>Err(receipt.lost_completion())}}},
            _=tokio::time::sleep_until(deadline)=>{if let Some(error)=receipt.fail(TransportError::ResponseTimeout) {Err(error)} else {match receiver.try_recv() {Ok(result)=>result,Err(_)=>Err(receipt.lost_completion())}}}
        }
    }
    /// 参数为普通请求或平面请求合并和传输期限；返回有效TC信封，包括Failed结果本身。
    /// 需要启用I/O与time的Tokio运行时；TC业务期限不由本参数修改。future取消同步移除等待项。
    /// 上层必须另行保留XID/BranchId及业务操作身份；本层只管理等待，不实现事务重试/协调。
    pub async fn request(&self, body: Message, timeout: Duration) -> Result<RpcMessage, CallError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .filter(|_| !timeout.is_zero())
            .ok_or_else(|| {
                CallError::local(
                    Some(self.generation()),
                    None,
                    TransportError::ResponseTimeout,
                )
            })?;
        self.request_until(body, deadline).await
    }
    /// 参数是请求和上层已建立的绝对截止；返回一次调用结果，锁等待/跨层派发不能重新起算期限。
    pub(crate) async fn request_until(
        &self,
        body: Message,
        deadline: Instant,
    ) -> Result<RpcMessage, CallError> {
        let generation = self.generation();
        let local = |id, reason| CallError::local(Some(generation), id, reason);
        if self.snapshot().closed {
            return Err(local(None, TransportError::ConnectionClosed));
        }
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(local(
                None,
                TransportError::InvalidConfiguration {
                    reason: "Tokio I/O and time runtime required",
                },
            ));
        }
        if deadline <= Instant::now() {
            return Err(local(None, TransportError::ResponseTimeout));
        }
        let expected =
            ResponseExpectation::from_request(&body, self.config.frames.payload_limits())
                .map_err(|reason| local(None, reason))?;
        let permit = Arc::clone(&self.permits)
            .try_acquire_owned()
            .map_err(|_| local(None, TransportError::Overloaded))?;
        let queue = match self.outbound.try_reserve() {
            Ok(value) => value,
            Err(mpsc::error::TrySendError::Full(_)) => {
                return Err(local(None, TransportError::Overloaded))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                return Err(local(None, TransportError::ConnectionClosed))
            }
        };
        let id = match self.core.next_id() {
            Ok(id) => id,
            Err(reason) => {
                // 额度用尽的旧代际已不可用于新请求；关闭以唤醒池监督，但绝不重发本调用。
                if matches!(reason, TransportError::RequestIdRotationRequired { .. }) {
                    self.core.terminate(reason.clone());
                }
                return Err(local(None, reason));
            }
        };
        let rpc = RpcMessage {
            id,
            message_type: RequestType::RequestSync,
            codec: SerializerType::Seata,
            compressor: CompressorType::None,
            head_map: Vec::new(),
            body: Some(body),
        };
        let bytes = ProtocolEncoderV1::new(self.config.frames)
            .encode(&rpc)
            .map_err(|reason| local(Some(id), reason.into()))?;
        let (reply, mut receiver) = oneshot::channel();
        let attempted = Arc::new(AtomicBool::new(false));
        self.core
            .register(
                id,
                PendingRequest {
                    expected,
                    deadline,
                    reply,
                    attempted: Arc::clone(&attempted),
                    _permit: permit,
                },
            )
            .map_err(|reason| local(Some(id), reason))?;
        let _guard = PendingRequestGuard::new(Arc::clone(&self.core), id);
        queue.send(OutboundCommand::Request { id, bytes });
        tokio::select! {
            biased;
            result=&mut receiver=>match result {Ok(value)=>value,Err(_)=>Err(self.lost_completion(id,&attempted))},
            _=tokio::time::sleep_until(deadline)=>{
                if let Some(error)=self.core.expire(id) {Err(error)} else {
                    // 完成者已经在同一锁边界内移交，权威响应或已决定的连接错误必须优先。
                    match receiver.try_recv() {Ok(value)=>value,Err(_)=>Err(self.lost_completion(id,&attempted))}
                }
            }
        }
    }
    fn lost_completion(&self, id: i32, attempted: &AtomicBool) -> CallError {
        let reason = TransportError::InternalState {
            reason: "pending completion channel disappeared",
        };
        self.core.terminate(reason.clone());
        CallError::request(
            self.generation(),
            id,
            attempted.load(Ordering::Acquire),
            reason,
        )
    }
}
