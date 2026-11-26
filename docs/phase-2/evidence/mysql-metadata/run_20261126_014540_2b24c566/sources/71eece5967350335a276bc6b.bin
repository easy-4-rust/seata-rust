//! 心跳调度与失活清理的真实loopback TCP验收；不宣称认证/TC业务状态。
#![allow(clippy::unwrap_used)]
#[path = "support/session_peer.rs"]
mod peer;
use peer::{begin, begin_result, config, eof, heartbeat, read, reply, write, xid};
use seata_protocol::{Message, RequestType};
use seata_transport::{CallError, ConnectionIdleConfig, TcpConnection, TransportError};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn enabled() -> seata_transport::ConnectionConfig {
    config(2).with_idle(
        ConnectionIdleConfig::new(Duration::from_millis(40), Duration::from_millis(240)).unwrap(),
    )
}

fn reset_config() -> seata_transport::ConnectionConfig {
    config(2).with_idle(
        ConnectionIdleConfig::new(Duration::from_millis(250), Duration::from_secs(2)).unwrap(),
    )
}

#[tokio::test]
async fn sustained_business_writes_reset_writer_idle_and_suppress_probes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.set_nodelay(true).unwrap();
        for _ in 0..7 {
            let request = read(&mut socket).await;
            assert_eq!(
                request.message_type,
                RequestType::RequestSync,
                "business activity should suppress scheduled PING"
            );
            write(&mut socket, reply(request.id, begin_result("busy"))).await;
        }
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, reset_config()).await.unwrap();
    let handle = connection.handle();
    for index in 0..7 {
        if index > 0 {
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
        assert_eq!(
            xid(handle
                .request(begin("busy"), Duration::from_secs(1))
                .await
                .unwrap()),
            "busy"
        );
    }
    assert_eq!(handle.snapshot().heartbeat_requests_sent, 0);
    assert_eq!(handle.snapshot().completed, 7);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn outbound_pong_replies_reset_writer_idle_and_suppress_own_probes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (done_tx, done_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.set_nodelay(true).unwrap();
        for id in 0..7 {
            if id > 0 {
                tokio::time::sleep(Duration::from_millis(60)).await;
            }
            write(&mut socket, heartbeat(id, RequestType::HeartbeatRequest)).await;
            let response = read(&mut socket).await;
            assert_eq!(
                response.message_type,
                RequestType::HeartbeatResponse,
                "PONG writes should suppress client PING"
            );
            assert_eq!(response.id, id);
        }
        done_tx.send(()).unwrap();
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, reset_config()).await.unwrap();
    let handle = connection.handle();
    done_rx.await.unwrap();
    assert_eq!(handle.snapshot().heartbeat_responses_sent, 7);
    assert_eq!(handle.snapshot().heartbeat_requests_sent, 0);
    assert_eq!(handle.snapshot().pending, 0);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[test]
fn idle_intervals_are_explicit_positive_ordered_and_checked() {
    for (write, read) in [
        (Duration::ZERO, Duration::from_secs(1)),
        (Duration::from_secs(1), Duration::ZERO),
        (Duration::from_secs(1), Duration::from_secs(1)),
        (Duration::from_secs(2), Duration::from_secs(1)),
        (Duration::from_secs(1), Duration::MAX),
    ] {
        assert!(ConnectionIdleConfig::new(write, read).is_err());
    }
    assert!(config(1).idle_config().is_none());
    let idle = ConnectionIdleConfig::new(Duration::from_secs(1), Duration::from_secs(3)).unwrap();
    assert_eq!(idle.writer_idle(), Duration::from_secs(1));
    assert_eq!(idle.reader_idle(), Duration::from_secs(3));
    assert_eq!(config(1).with_idle(idle).idle_config(), Some(idle));
}

#[tokio::test]
async fn writer_idle_sends_native_ping_without_pending_and_business_id_is_next() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (ping_tx, ping_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.set_nodelay(true).unwrap();
        let ping = read(&mut socket).await;
        assert_eq!(ping.message_type, RequestType::HeartbeatRequest);
        assert!(ping.body.is_none());
        assert_eq!(ping.id, 1);
        write(
            &mut socket,
            heartbeat(ping.id, RequestType::HeartbeatResponse),
        )
        .await;
        ping_tx.send(()).unwrap();
        loop {
            let request = read(&mut socket).await;
            if request.message_type == RequestType::HeartbeatRequest {
                write(
                    &mut socket,
                    heartbeat(request.id, RequestType::HeartbeatResponse),
                )
                .await;
                continue;
            }
            assert!(request.id > ping.id);
            assert!(matches!(request.body, Some(Message::GlobalBeginRequest(_))));
            write(&mut socket, reply(request.id, begin_result("after-ping"))).await;
            break;
        }
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, enabled()).await.unwrap();
    let handle = connection.handle();
    ping_rx.await.unwrap();
    assert_eq!(handle.snapshot().pending, 0);
    assert_eq!(handle.snapshot().write_attempted, 0);
    assert_eq!(
        xid(handle
            .request(begin("business"), Duration::from_secs(1))
            .await
            .unwrap()),
        "after-ping"
    );
    assert!(handle.snapshot().heartbeat_requests_sent >= 1);
    assert_eq!(handle.snapshot().completed, 1);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn repeated_pongs_keep_idle_session_alive_without_creating_waiters() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (ready_tx, ready_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.set_nodelay(true).unwrap();
        let mut previous = None;
        // 总时间超过reader_idle，必须依靠有效PONG持续刷新。
        for _ in 0..9 {
            let ping = read(&mut socket).await;
            assert_eq!(ping.message_type, RequestType::HeartbeatRequest);
            assert_ne!(previous, Some(ping.id));
            previous = Some(ping.id);
            write(
                &mut socket,
                heartbeat(ping.id, RequestType::HeartbeatResponse),
            )
            .await;
        }
        ready_tx.send(()).unwrap();
        loop {
            let value = read(&mut socket).await;
            if value.message_type == RequestType::HeartbeatRequest {
                write(
                    &mut socket,
                    heartbeat(value.id, RequestType::HeartbeatResponse),
                )
                .await;
            } else {
                write(&mut socket, reply(value.id, peer::status_result())).await;
                break;
            }
        }
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, enabled()).await.unwrap();
    let handle = connection.handle();
    ready_rx.await.unwrap();
    assert!(!handle.snapshot().closed);
    assert_eq!(handle.snapshot().pending, 0);
    assert!(handle.snapshot().heartbeat_requests_sent >= 9);
    let result = handle
        .request(
            Message::GlobalStatusRequest(seata_protocol::transaction::GlobalStatusRequest {
                xid: Some("known-after-nine".into()),
                extra_data: None,
            }),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
    assert!(matches!(
        result.body,
        Some(Message::GlobalStatusResponse(_))
    ));
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn silent_peer_idle_expiry_is_unknown_for_observed_request_and_closes_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let first = read(&mut socket).await;
        assert!(matches!(first.body, Some(Message::GlobalBeginRequest(_))));
        let mut bytes = vec![];
        tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut bytes))
            .await
            .unwrap()
            .unwrap();
        assert!(!bytes.is_empty());
        let decoder = seata_protocol::v1::ProtocolDecoderV1::new(peer::limits());
        let mut rest = bytes.as_slice();
        let mut count = 0;
        while !rest.is_empty() {
            let len = decoder.frame_length(rest).unwrap().unwrap();
            let frame = decoder.decode(&rest[..len]).unwrap();
            assert_eq!(frame.message_type, RequestType::HeartbeatRequest);
            assert!(frame.body.is_none());
            rest = &rest[len..];
            count += 1;
        }
        assert!(count >= 1);
    });
    let connection = TcpConnection::connect(addr, enabled()).await.unwrap();
    let handle = connection.handle();
    assert!(matches!(
        handle
            .request(begin("no-replay"), Duration::from_secs(2))
            .await,
        Err(CallError::UnknownOutcome {
            reason: TransportError::ReadIdleTimeout,
            ..
        })
    ));
    assert!(handle.snapshot().closed);
    assert_eq!(handle.snapshot().pending, 0);
    assert_eq!(handle.snapshot().read_idle_expirations, 1);
    assert_eq!(handle.snapshot().write_attempted, 1);
    assert!(matches!(
        handle
            .request(begin("closed"), Duration::from_secs(1))
            .await,
        Err(CallError::LocalFailureBeforeSend { .. })
    ));
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn partial_frame_drips_cannot_keep_a_generation_alive() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.set_nodelay(true).unwrap();
        read(&mut socket).await;
        let bytes = seata_protocol::v1::ProtocolEncoderV1::new(peer::limits())
            .encode(&heartbeat(77, RequestType::HeartbeatResponse))
            .unwrap();
        // 持续滴流超过idle上限；错误地按TCP字节刷新期限会一直存活并让该测试失败。
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        let mut tick = tokio::time::interval(Duration::from_millis(25));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut sent = 0;
        let mut sink = [0; 1024];
        loop {
            tokio::select! {
                _=tokio::time::sleep_until(deadline)=>panic!("partial-frame drips kept the connection alive"),
                result=socket.read(&mut sink)=>if result.unwrap()==0 {break;},
                _=tick.tick()=>{
                    if socket.write_all(&[bytes[sent%bytes.len()]]).await.is_err() {break;}
                    sent+=1;
                }
            }
        }
        assert!(
            sent < bytes.len(),
            "idle must expire before the drip completes one frame"
        );
    });
    let connection = TcpConnection::connect(addr, enabled()).await.unwrap();
    let handle = connection.handle();
    assert!(matches!(
        handle
            .request(begin("partial"), Duration::from_secs(2))
            .await,
        Err(CallError::UnknownOutcome {
            reason: TransportError::ReadIdleTimeout,
            ..
        })
    ));
    assert_eq!(handle.snapshot().heartbeats_received, 0);
    assert_eq!(handle.snapshot().read_idle_expirations, 1);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn heartbeat_ids_exhaust_without_reuse_and_require_new_generation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.set_nodelay(true).unwrap();
        for id in [i32::MAX, i32::MIN] {
            let ping = read(&mut socket).await;
            assert_eq!(ping.id, id);
            assert_eq!(ping.message_type, RequestType::HeartbeatRequest);
            write(&mut socket, heartbeat(id, RequestType::HeartbeatResponse)).await;
        }
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, enabled().with_id_limits(i32::MAX, 2).unwrap())
        .await
        .unwrap();
    let handle = connection.handle();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !handle.snapshot().closed {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        handle.snapshot().last_failure,
        Some(TransportError::RequestIdRotationRequired {
            issued: 2,
            maximum: 2
        })
    ));
    assert_eq!(handle.snapshot().heartbeat_requests_sent, 2);
    assert_eq!(handle.snapshot().pending, 0);
    assert_eq!(handle.snapshot().write_attempted, 0);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn disabled_idle_emits_no_probe_and_owner_drop_stops_enabled_timers() {
    for active in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            eof(&mut socket).await;
        });
        let connection = TcpConnection::connect(addr, if active { enabled() } else { config(1) })
            .await
            .unwrap();
        let handle = connection.handle();
        if !active {
            tokio::time::sleep(Duration::from_millis(120)).await;
            assert!(!handle.snapshot().closed);
        }
        drop(connection);
        server.await.unwrap();
        let before = handle.snapshot();
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(handle.snapshot(), before);
        assert_eq!(before.heartbeat_requests_sent, 0);
        assert_eq!(before.read_idle_expirations, 0);
    }
}
