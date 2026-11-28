//! 虚拟单调时钟驱动真实writer循环，验证跳过的队列项不能刷新活动时钟。
#![allow(clippy::unwrap_used)]
use super::write_loop;
use crate::netty::outbound_command::OutboundCommand;
use crate::netty::pending_requests::PendingRequests;
use crate::netty::worker_exit_guard::WorkerExitGuard;
use crate::{ConnectionConfig, ConnectionGeneration, ConnectionIdleConfig, TransportError};
use seata_protocol::v1::{FrameLimits, ProtocolDecoderV1};
use seata_protocol::{PayloadLimits, RequestType};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, watch};

/// writer测试伴随夹具，集中保存现有I/O、回执预算和协议预期，不进入生产代码。
/// 对应 Java: io.netty.channel.ChannelPromise的测试上下文职责。
struct ReplyFixture {
    config: ConnectionConfig,
    core: Arc<PendingRequests>,
    handle: crate::ConnectionHandle,
    rx: mpsc::Receiver<OutboundCommand>,
    stop: watch::Receiver<bool>,
    request: seata_protocol::protocol::RpcMessage,
    body: seata_protocol::Message,
    expected: Vec<u8>,
}
fn reply_setup() -> ReplyFixture {
    use seata_protocol::protocol::RpcMessage;
    use seata_protocol::transaction::{
        AbstractTransactionResponse, GlobalBeginRequest, GlobalBeginResponse,
    };
    use seata_protocol::{
        CompressorType, Message, ResultCode, SerializerType, TransactionExceptionCode,
    };
    let frames = FrameLimits::new(
        65536,
        32767,
        128,
        PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let config =
        ConnectionConfig::new(frames, 1, Duration::from_secs(1), Duration::from_secs(1)).unwrap();
    let (shutdown, stop) = watch::channel(false);
    let core = Arc::new(
        PendingRequests::new(ConnectionGeneration::next().unwrap(), config, shutdown).unwrap(),
    );
    let (tx, rx) = mpsc::channel(2);
    let handle = crate::ConnectionHandle {
        core: Arc::clone(&core),
        outbound: tx,
        permits: Arc::new(tokio::sync::Semaphore::new(1)),
        response_permits: Arc::new(tokio::sync::Semaphore::new(1)),
        config,
    };
    let request = RpcMessage {
        id: -9,
        message_type: RequestType::RequestSync,
        codec: SerializerType::Seata,
        compressor: CompressorType::None,
        head_map: vec![],
        body: Some(Message::GlobalBeginRequest(GlobalBeginRequest {
            timeout: 30000,
            transaction_name: Some("inbound".into()),
        })),
    };
    let body = Message::GlobalBeginResponse(GlobalBeginResponse {
        response: AbstractTransactionResponse {
            result_code: ResultCode::Success,
            msg: None,
            transaction_exception_code: TransactionExceptionCode::Unknown,
        },
        xid: Some("x".repeat(256)),
        extra_data: None,
    });
    let mut reply = request.clone();
    reply.message_type = RequestType::Response;
    reply.body = Some(body.clone());
    let expected = seata_protocol::v1::ProtocolEncoderV1::new(frames)
        .encode(&reply)
        .unwrap();
    ReplyFixture {
        config,
        core,
        handle,
        rx,
        stop,
        request,
        body,
        expected,
    }
}

#[tokio::test(start_paused = true)]
async fn response_partial_write_survives_deadline_or_cancellation_and_closure_never_reuses_stream()
{
    for mode in 0..3 {
        let ReplyFixture {
            config,
            core,
            handle,
            rx,
            stop,
            request,
            body,
            mut expected,
        } = reply_setup();
        let (writer, mut peer) = tokio::io::duplex(8);
        let guard = WorkerExitGuard::new(Arc::clone(&core));
        let worker = tokio::spawn(write_loop(
            writer,
            Arc::clone(&core),
            rx,
            stop,
            config,
            guard,
        ));
        let caller = handle.clone();
        let call = tokio::spawn(async move {
            caller
                .respond(&request, body, Duration::from_millis(100))
                .await
        });
        while core.snapshot().response_write_attempted == 0 {
            tokio::task::yield_now().await;
        }
        assert_eq!(core.snapshot().responses_written, 0);
        assert!(!call.is_finished());
        if mode == 0 {
            tokio::time::advance(Duration::from_millis(100)).await;
            assert!(matches!(
                call.await.unwrap(),
                Err(crate::CallError::UnknownOutcome {
                    reason: TransportError::ResponseTimeout,
                    ..
                })
            ));
        } else if mode == 1 {
            call.abort();
            assert!(call.await.unwrap_err().is_cancelled());
        } else {
            core.terminate(TransportError::ReadIdleTimeout);
            assert!(matches!(
                call.await.unwrap(),
                Err(crate::CallError::UnknownOutcome {
                    reason: TransportError::ReadIdleTimeout,
                    ..
                })
            ));
            worker.await.unwrap();
            let mut partial = vec![];
            peer.read_to_end(&mut partial).await.unwrap();
            assert_eq!(partial, expected[..8]);
            assert_eq!(handle.response_permits.available_permits(), 1);
            continue;
        }
        let pong = seata_protocol::protocol::RpcMessage {
            id: 33,
            message_type: RequestType::HeartbeatResponse,
            codec: seata_protocol::SerializerType::Seata,
            compressor: seata_protocol::CompressorType::None,
            head_map: vec![],
            body: None,
        };
        let bytes = seata_protocol::v1::ProtocolEncoderV1::new(config.frames)
            .encode(&pong)
            .unwrap();
        expected.extend_from_slice(&bytes);
        handle
            .outbound
            .send(OutboundCommand::Control { bytes })
            .await
            .unwrap();
        let mut actual = vec![0; expected.len()];
        peer.read_exact(&mut actual).await.unwrap();
        assert_eq!(
            actual, expected,
            "later frame cannot replace an unfinished response prefix"
        );
        assert_eq!(core.snapshot().responses_written, 1);
        assert_eq!(core.snapshot().pending, 0);
        assert_eq!(handle.response_permits.available_permits(), 1);
        core.terminate(TransportError::ConnectionClosed);
        worker.await.unwrap();
    }
}

#[tokio::test(start_paused = true)]
async fn response_queue_budget_and_prewrite_expiry_or_cancel_skip_all_reply_bytes() {
    for cancel in [false, true] {
        let ReplyFixture {
            config,
            core,
            handle,
            rx,
            stop,
            request,
            body,
            ..
        } = reply_setup();
        let caller = handle.clone();
        let other = request.clone();
        let second = body.clone();
        let call = tokio::spawn(async move {
            caller
                .respond(&request, body, Duration::from_millis(100))
                .await
        });
        while handle.outbound.capacity() != 1 {
            tokio::task::yield_now().await;
        }
        assert!(matches!(
            handle.respond(&other, second, Duration::from_secs(1)).await,
            Err(crate::CallError::LocalFailureBeforeSend {
                reason: TransportError::Overloaded,
                ..
            })
        ));
        if cancel {
            call.abort();
            assert!(call.await.unwrap_err().is_cancelled());
        } else {
            tokio::time::advance(Duration::from_millis(100)).await;
            assert!(matches!(
                call.await.unwrap(),
                Err(crate::CallError::LocalFailureBeforeSend {
                    reason: TransportError::ResponseTimeout,
                    ..
                })
            ));
        }
        let (writer, mut peer) = tokio::io::duplex(1024);
        let guard = WorkerExitGuard::new(Arc::clone(&core));
        let worker = tokio::spawn(write_loop(
            writer,
            Arc::clone(&core),
            rx,
            stop,
            config,
            guard,
        ));
        let pong = seata_protocol::protocol::RpcMessage {
            id: 1,
            message_type: RequestType::HeartbeatResponse,
            codec: seata_protocol::SerializerType::Seata,
            compressor: seata_protocol::CompressorType::None,
            head_map: vec![],
            body: None,
        };
        let expected = seata_protocol::v1::ProtocolEncoderV1::new(config.frames)
            .encode(&pong)
            .unwrap();
        handle
            .outbound
            .send(OutboundCommand::Control {
                bytes: expected.clone(),
            })
            .await
            .unwrap();
        let mut actual = vec![0; expected.len()];
        peer.read_exact(&mut actual).await.unwrap();
        assert_eq!(actual, expected);
        assert_eq!(core.snapshot().response_write_attempted, 0);
        assert_eq!(core.snapshot().responses_written, 0);
        assert_eq!(handle.response_permits.available_permits(), 1);
        core.terminate(TransportError::ConnectionClosed);
        worker.await.unwrap();
        let mut remaining = vec![];
        peer.read_to_end(&mut remaining).await.unwrap();
        assert!(remaining.is_empty());
    }
}

#[tokio::test]
async fn dropping_unpolled_response_writer_resolves_receipt_and_releases_budget() {
    let ReplyFixture {
        config,
        core,
        handle,
        rx,
        stop,
        request,
        body,
        ..
    } = reply_setup();
    let caller = handle.clone();
    let call =
        tokio::spawn(async move { caller.respond(&request, body, Duration::from_secs(5)).await });
    while handle.outbound.capacity() != 1 {
        tokio::task::yield_now().await;
    }
    let (writer, _peer) = tokio::io::duplex(8);
    let guard = WorkerExitGuard::new(Arc::clone(&core));
    let unpolled = write_loop(writer, Arc::clone(&core), rx, stop, config, guard);
    drop(unpolled);
    assert!(matches!(
        call.await.unwrap(),
        Err(crate::CallError::LocalFailureBeforeSend { .. })
    ));
    assert!(core.snapshot().closed);
    assert_eq!(handle.response_permits.available_permits(), 1);
}

#[tokio::test(start_paused = true)]
async fn skipped_cancelled_queue_entries_do_not_reset_writer_idle() {
    let frames = FrameLimits::new(
        65536,
        32767,
        128,
        PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let config = ConnectionConfig::new(frames, 2, Duration::from_secs(1), Duration::from_secs(1))
        .unwrap()
        .with_idle(
            ConnectionIdleConfig::new(Duration::from_millis(10), Duration::from_millis(100))
                .unwrap(),
        );
    let (shutdown, stop) = watch::channel(false);
    let core = Arc::new(
        PendingRequests::new(ConnectionGeneration::next().unwrap(), config, shutdown).unwrap(),
    );
    let (tx, rx) = mpsc::channel(2);
    let (writer, mut peer) = tokio::io::duplex(1024);
    let guard = WorkerExitGuard::new(Arc::clone(&core));
    let task = tokio::spawn(write_loop(
        writer,
        Arc::clone(&core),
        rx,
        stop,
        config,
        guard,
    ));
    tokio::task::yield_now().await;
    // 没有等待项，等价于在首次write轮询前取消后的队列残项；若错误写入则无效字节破坏帧。
    for id in 100..104 {
        tokio::time::advance(Duration::from_millis(2)).await;
        tx.send(OutboundCommand::Request {
            id,
            bytes: vec![255],
        })
        .await
        .unwrap();
        while tx.capacity() != 2 {
            tokio::task::yield_now().await;
        }
    }
    tokio::time::advance(Duration::from_millis(2)).await;
    let mut bytes = [0; 16];
    tokio::time::timeout(Duration::from_millis(1), peer.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    let ping = ProtocolDecoderV1::new(frames).decode(&bytes).unwrap();
    assert_eq!(ping.id, 1);
    assert_eq!(ping.message_type, RequestType::HeartbeatRequest);
    assert_eq!(core.snapshot().heartbeat_requests_sent, 1);
    assert_eq!(core.snapshot().write_attempted, 0);
    assert_eq!(core.snapshot().pending, 0);
    core.terminate(TransportError::ConnectionClosed);
    task.await.unwrap();
    let mut remaining = vec![];
    peer.read_to_end(&mut remaining).await.unwrap();
    assert!(remaining.is_empty());
}

#[tokio::test(start_paused = true)]
async fn buffered_response_is_not_complete_until_flush_drains_the_original_writer() {
    let ReplyFixture {
        config,
        core,
        handle,
        rx,
        stop,
        request,
        body,
        expected,
    } = reply_setup();
    let (stream, mut peer) = tokio::io::duplex(1);
    let writer = tokio::io::BufWriter::with_capacity(65536, stream);
    let guard = WorkerExitGuard::new(core.clone());
    let worker = tokio::spawn(write_loop(writer, core.clone(), rx, stop, config, guard));
    let call =
        tokio::spawn(async move { handle.respond(&request, body, Duration::from_secs(1)).await });
    while core.snapshot().response_write_attempted == 0 {
        tokio::task::yield_now().await;
    }
    assert!(
        !call.is_finished(),
        "buffered plaintext acceptance cannot publish a completed response"
    );
    assert_eq!(core.snapshot().responses_written, 0);
    let mut actual = vec![0; expected.len()];
    peer.read_exact(&mut actual).await.unwrap();
    assert_eq!(actual, expected);
    call.await.unwrap().unwrap();
    assert_eq!(core.snapshot().responses_written, 1);
    core.terminate(TransportError::ConnectionClosed);
    worker.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn blocked_flush_expires_the_same_write_budget_and_keeps_response_unknown() {
    let ReplyFixture {
        mut config,
        core,
        handle,
        rx,
        stop,
        request,
        body,
        ..
    } = reply_setup();
    config.write_timeout = Duration::from_millis(30);
    let (stream, _peer) = tokio::io::duplex(1);
    let writer = tokio::io::BufWriter::with_capacity(65536, stream);
    let guard = WorkerExitGuard::new(core.clone());
    let worker = tokio::spawn(write_loop(writer, core.clone(), rx, stop, config, guard));
    let call =
        tokio::spawn(async move { handle.respond(&request, body, Duration::from_secs(1)).await });
    while core.snapshot().response_write_attempted == 0 {
        tokio::task::yield_now().await;
    }
    assert!(
        !call.is_finished(),
        "buffered response must wait for the physical writer"
    );
    tokio::time::advance(Duration::from_millis(30)).await;
    assert!(matches!(
        call.await.unwrap(),
        Err(crate::CallError::UnknownOutcome {
            reason: TransportError::WriteTimeout,
            ..
        })
    ));
    assert_eq!(core.snapshot().responses_written, 0);
    assert!(core.snapshot().closed);
    worker.await.unwrap();
}
