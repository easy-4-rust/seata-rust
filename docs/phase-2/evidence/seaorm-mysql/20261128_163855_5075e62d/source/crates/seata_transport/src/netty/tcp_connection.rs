use super::connection_handle::ConnectionHandle;
use super::idle_timer::IdleTimer;
use super::outbound_command::OutboundCommand;
use super::pending_requests::PendingRequests;
use super::worker_exit_guard::WorkerExitGuard;
use crate::{CallError, ConnectionConfig, ConnectionGeneration, FrameDecoder, TransportError};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::v1::ProtocolEncoderV1;
use seata_protocol::{CompressorType, Message, RequestType, SerializerType};
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// 独占一个TCP会话、接收队列和后台任务；不是已认证/资源注册完成的ready状态。
/// 对应 Java: io.netty.channel.Channel 的单连接I/O与关闭所有权；Rust显式监督任务。
pub struct TcpConnection {
    handle: ConnectionHandle,
    inbound: mpsc::Receiver<RpcMessage>,
    tasks: Option<(JoinHandle<()>, JoinHandle<()>)>,
}
impl fmt::Debug for TcpConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpConnection")
            .field("handle", &self.handle)
            .field("queued_inbound", &self.inbound.len())
            .finish()
    }
}
impl TcpConnection {
    /// 参数为数值socket地址及已验证预算；在启用I/O/time的Tokio运行时建立一次真实TCP连接。
    /// 返回拥有任务的会话；失败时没有请求帧写入，不创建事务重试或身份注册流程。
    pub async fn connect(address: SocketAddr, config: ConnectionConfig) -> Result<Self, CallError> {
        let generation =
            ConnectionGeneration::next().map_err(|reason| CallError::local(None, None, reason))?;
        let local = |reason| CallError::local(Some(generation), None, reason);
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(local(TransportError::InvalidConfiguration {
                reason: "Tokio I/O and time runtime required",
            }));
        }
        let deadline = Instant::now()
            .checked_add(config.connect_timeout)
            .ok_or_else(|| {
                local(TransportError::InvalidConfiguration {
                    reason: "connect deadline overflow",
                })
            })?;
        let stream = match tokio::time::timeout_at(deadline, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(error)) => return Err(local(TransportError::Io { kind: error.kind() })),
            Err(_) => return Err(local(TransportError::ConnectTimeout)),
        };
        stream
            .set_nodelay(true)
            .map_err(|error| local(TransportError::Io { kind: error.kind() }))?;
        let (reader, writer) = stream.into_split();
        Self::from_halves(reader, writer, generation, config)
    }
    /// 参数为原端点、预算及显式 mTLS 配置；返回验证后的 TLS 1.3 会话，失败绝不回退明文。
    /// 同一绝对连接预算覆盖 TCP 与握手；失败时没有任何 V1 帧或请求编号。
    #[cfg(feature = "tls")]
    pub async fn connect_tls(
        address: SocketAddr,
        config: ConnectionConfig,
        tls: &crate::TlsClientConfig,
    ) -> Result<Self, CallError> {
        let generation =
            ConnectionGeneration::next().map_err(|reason| CallError::local(None, None, reason))?;
        let local = |reason| CallError::local(Some(generation), None, reason);
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(local(TransportError::InvalidConfiguration {
                reason: "Tokio I/O and time runtime required",
            }));
        }
        let deadline = Instant::now()
            .checked_add(config.connect_timeout)
            .ok_or_else(|| {
                local(TransportError::InvalidConfiguration {
                    reason: "connect deadline overflow",
                })
            })?;
        let stream = match tokio::time::timeout_at(deadline, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(error)) => return Err(local(TransportError::Io { kind: error.kind() })),
            Err(_) => return Err(local(TransportError::ConnectTimeout)),
        };
        stream
            .set_nodelay(true)
            .map_err(|error| local(TransportError::Io { kind: error.kind() }))?;
        let stream = tls.connect(stream, deadline).await.map_err(local)?;
        let (reader, writer) = tokio::io::split(stream);
        Self::from_halves(reader, writer, generation, config)
    }
    fn from_halves<R, W>(
        reader: R,
        writer: W,
        generation: ConnectionGeneration,
        config: ConnectionConfig,
    ) -> Result<Self, CallError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let local = |reason| CallError::local(Some(generation), None, reason);
        let (shutdown, stop) = watch::channel(false);
        let core = Arc::new(PendingRequests::new(generation, config, shutdown).map_err(local)?);
        let (outbound, commands) = mpsc::channel(config.maximum_pending + 1);
        let (incoming, inbound) = mpsc::channel(config.maximum_pending);
        let handle = ConnectionHandle {
            core: Arc::clone(&core),
            outbound: outbound.clone(),
            permits: Arc::new(Semaphore::new(config.maximum_pending)),
            response_permits: Arc::new(Semaphore::new(config.maximum_pending)),
            config,
        };
        // 守卫作为future参数在首次poll前已构造，任务从未poll就被abort也能传播关闭。
        let read_exit = WorkerExitGuard::new(Arc::clone(&core));
        let write_exit = WorkerExitGuard::new(Arc::clone(&core));
        let read_task = tokio::spawn(read_loop(
            reader,
            Arc::clone(&core),
            outbound,
            incoming,
            stop.clone(),
            config,
            read_exit,
        ));
        let write_task = tokio::spawn(write_loop(writer, core, commands, stop, config, write_exit));
        Ok(Self {
            handle,
            inbound,
            tasks: Some((read_task, write_task)),
        })
    }
    /// 返回可克隆的调用句柄；它始终绑定本连接，不跟随替换通道迁移。
    pub fn handle(&self) -> ConnectionHandle {
        self.handle.clone()
    }
    /// 返回此连接已接收的入站请求；单消费者，关闭后仍可取出先前合法接收的队列项。
    /// 本层不执行RM业务或发送响应，认证/分发/业务ready由上层负责。
    pub async fn next_inbound(&mut self) -> Option<RpcMessage> {
        self.inbound.recv().await
    }
    /// 返回可消费一次回包的原代际入站能力；与next_inbound共享单消费者队列。
    pub async fn next_request(&mut self) -> Option<crate::PoolInbound> {
        self.inbound.recv().await.map(|message| crate::PoolInbound {
            connection: self.handle.clone(),
            message,
        })
    }
    /// 消费所有者，关闭该代际、清理等待项并等待两个任务结束；本future被取消时Drop仍会abort任务。
    pub async fn shutdown(mut self) -> Result<(), TransportError> {
        self.handle.core.terminate(TransportError::ConnectionClosed);
        let mut failed = false;
        if let Some((reader, writer)) = self.tasks.as_mut() {
            if reader.await.is_err() {
                failed = true;
            }
            if writer.await.is_err() {
                failed = true;
            }
        }
        self.tasks = None;
        if failed {
            Err(TransportError::InternalState {
                reason: "owned connection task failed",
            })
        } else {
            Ok(())
        }
    }
}
impl Drop for TcpConnection {
    fn drop(&mut self) {
        self.handle.core.terminate(TransportError::ConnectionClosed);
        if let Some((reader, writer)) = &self.tasks {
            reader.abort();
            writer.abort();
        }
    }
}

async fn read_loop<R: AsyncRead + Unpin>(
    mut reader: R,
    core: Arc<PendingRequests>,
    outbound: mpsc::Sender<OutboundCommand>,
    incoming: mpsc::Sender<RpcMessage>,
    mut stop: watch::Receiver<bool>,
    config: ConnectionConfig,
    _exit: WorkerExitGuard,
) {
    if *stop.borrow() {
        return;
    }
    let mut decoder = FrameDecoder::new(config.frames);
    let mut buffer = vec![0; config.frames.maximum_frame_length().min(16 * 1024)];
    let mut idle = IdleTimer::new(config.idle.map(|value| value.reader_idle()));
    loop {
        let count = tokio::select! {biased; _=stop.changed()=>return,
            result=idle.wait()=>{core.terminate(result.err().unwrap_or(TransportError::ReadIdleTimeout));return;},
            result=reader.read(&mut buffer)=>match result {
            Ok(0)=>{let reason=decoder.finish().err().unwrap_or(TransportError::ConnectionClosed);core.terminate(reason);return;},
            Ok(count)=>count,Err(error)=>{core.terminate(TransportError::Io {kind:error.kind()});return;}
        }};
        let mut input = &buffer[..count];
        while !input.is_empty() {
            if core.snapshot().closed {
                return;
            }
            let frame = match decoder.decode(&mut input) {
                Ok(Some(value)) => value,
                Ok(None) => break,
                Err(error) => {
                    core.terminate(error);
                    return;
                }
            };
            match idle.expired() {
                Ok(false) => {}
                Ok(true) => {
                    core.terminate(TransportError::ReadIdleTimeout);
                    return;
                }
                Err(reason) => {
                    core.terminate(reason);
                    return;
                }
            }
            // 仅完整且协议有效的帧刷新读空闲，恶意半帧滴流不能让期限无限延期。
            idle.reset();
            match frame.message_type {
                RequestType::Response => core.complete(frame),
                RequestType::HeartbeatRequest => {
                    core.heartbeat();
                    let pong = RpcMessage {
                        id: frame.id,
                        message_type: RequestType::HeartbeatResponse,
                        codec: SerializerType::Seata,
                        compressor: CompressorType::None,
                        head_map: frame.head_map,
                        body: None,
                    };
                    let bytes = match ProtocolEncoderV1::new(config.frames).encode(&pong) {
                        Ok(value) => value,
                        Err(error) => {
                            core.terminate(error.into());
                            return;
                        }
                    };
                    if outbound
                        .try_send(OutboundCommand::Control { bytes })
                        .is_err()
                    {
                        core.terminate(TransportError::Overloaded);
                        return;
                    }
                }
                RequestType::HeartbeatResponse => core.heartbeat(),
                RequestType::RequestSync | RequestType::RequestOneway => {
                    if frame.body.as_ref().is_some_and(|body| {
                        body.is_result() || matches!(body, Message::MergeResultMessage(_))
                    }) {
                        core.terminate(TransportError::UnexpectedResponse {
                            reason: "result payload arrived in a request envelope",
                        });
                        return;
                    }
                    if incoming.try_send(frame).is_err() {
                        core.terminate(TransportError::Overloaded);
                        return;
                    }
                    core.inbound();
                }
            }
        }
    }
}

async fn write_loop<W: AsyncWrite + Unpin>(
    mut writer: W,
    core: Arc<PendingRequests>,
    mut commands: mpsc::Receiver<OutboundCommand>,
    mut stop: watch::Receiver<bool>,
    config: ConnectionConfig,
    _exit: WorkerExitGuard,
) {
    if *stop.borrow() {
        return;
    }
    let mut idle = IdleTimer::new(config.idle.map(|value| value.writer_idle()));
    loop {
        let command = tokio::select! {biased; _=stop.changed()=>return,
        result=idle.wait()=>match result.and_then(|()|scheduled_probe(&core,config)) {Ok(value)=>value,Err(reason)=>{core.terminate(reason);return;}},
        value=commands.recv()=>match value {Some(value)=>value,None=>return}};
        let deadline = match Instant::now().checked_add(config.write_timeout) {
            Some(value) => value,
            None => {
                core.terminate(TransportError::InvalidConfiguration {
                    reason: "write deadline overflow",
                });
                return;
            }
        };
        let write = async {
            let bytes = match &command {
                OutboundCommand::Request { id, bytes } => {
                    if !core.mark_write_attempt(*id) {
                        return Ok(false);
                    }
                    bytes
                }
                OutboundCommand::Control { bytes } | OutboundCommand::Probe { bytes } => bytes,
                OutboundCommand::Response(response) => {
                    if !response.receipt.begin() {
                        return Ok(false);
                    }
                    &response.bytes
                }
            };
            writer.write_all(bytes).await?;
            // TLS/缓冲流接受明文不等于密文已写出；flush 共享原写期限，完成后才发布回执。
            writer.flush().await?;
            Ok::<bool, std::io::Error>(true)
        };
        let result = tokio::select! {biased; _=stop.changed()=>return, result=tokio::time::timeout_at(deadline,write)=>result};
        match result {
            Ok(Ok(written)) => {
                if written {
                    idle.reset();
                    match command {
                        OutboundCommand::Probe { .. } => core.heartbeat_written(true),
                        OutboundCommand::Control { .. } => core.heartbeat_written(false),
                        OutboundCommand::Request { .. } => {}
                        OutboundCommand::Response(response) => response.receipt.written(),
                    }
                }
            }
            Ok(Err(error)) => {
                core.terminate(TransportError::Io { kind: error.kind() });
                return;
            }
            Err(_) => {
                core.terminate(TransportError::WriteTimeout);
                return;
            }
        }
    }
}

fn scheduled_probe(
    core: &PendingRequests,
    config: ConnectionConfig,
) -> Result<OutboundCommand, TransportError> {
    // 与业务请求共享发号空间，但心跳不会创建事务future或触发业务重放。
    let frame = RpcMessage {
        id: core.next_id()?,
        message_type: RequestType::HeartbeatRequest,
        codec: SerializerType::Seata,
        compressor: CompressorType::None,
        head_map: Vec::new(),
        body: None,
    };
    Ok(OutboundCommand::Probe {
        bytes: ProtocolEncoderV1::new(config.frames).encode(&frame)?,
    })
}

#[cfg(test)]
#[path = "tcp_connection_tests.rs"]
mod tests;
