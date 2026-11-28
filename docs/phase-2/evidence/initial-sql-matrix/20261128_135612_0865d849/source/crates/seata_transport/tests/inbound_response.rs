//! 原连接回包能力及响应写入完成语义；不模拟持久SQL补偿。
#![allow(clippy::unwrap_used)]
#[path = "support/session_peer.rs"]
mod peer;
use peer::{begin, begin_result, config, eof, read, reply, write};
use seata_protocol::protocol::{MergeResultMessage, MergedWarpMessage, RpcMessage};
use seata_protocol::transaction::{
    BranchCommitRequest, BranchCommitResponse, BranchRollbackRequest, BranchRollbackResponse,
};
use seata_protocol::{BranchStatus, BranchType, Message, RequestType};
use seata_transport::{
    CallError, ConnectionPool, ConnectionPoolConfig, ReconnectPolicy, TcpConnection, TransportError,
};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn callback(id: i32, commit: bool) -> RpcMessage {
    let body = if commit {
        Message::BranchCommitRequest(BranchCommitRequest {
            xid: "original-xid".into(),
            branch_id: 9007199254740993,
            branch_type: BranchType::At,
            resource_id: "mysql://original".into(),
            application_data: Some("exact-operation".into()),
        })
    } else {
        Message::BranchRollbackRequest(BranchRollbackRequest {
            xid: "original-xid".into(),
            branch_id: 9007199254740993,
            branch_type: BranchType::At,
            resource_id: "mysql://original".into(),
            application_data: Some("exact-operation".into()),
        })
    };
    let mut frame = reply(id, body);
    frame.message_type = RequestType::RequestSync;
    frame.head_map = vec![
        ("trace".into(), Some("original".into())),
        ("nullable".into(), None),
        ("empty".into(), Some(String::new())),
    ];
    frame
}
fn response(commit: bool) -> Message {
    if commit {
        Message::BranchCommitResponse(BranchCommitResponse {
            response: peer::success(),
            xid: Some("original-xid".into()),
            branch_id: 9007199254740993,
            branch_status: BranchStatus::PhaseTwoCommitted,
        })
    } else {
        Message::BranchRollbackResponse(BranchRollbackResponse {
            response: peer::success(),
            xid: Some("original-xid".into()),
            branch_id: 9007199254740993,
            branch_status: BranchStatus::PhaseTwoRollbacked,
        })
    }
}

#[test]
fn reply_without_tokio_context_is_typed_local_failure_before_queue_or_write() {
    use std::future::Future;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (connection, inbound, server) = runtime.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            write(&mut socket, callback(1, true)).await;
            eof(&mut socket).await;
        });
        let mut connection = TcpConnection::connect(address, config(1)).await.unwrap();
        let inbound = connection.next_request().await.unwrap();
        (connection, inbound, server)
    });
    let handle = connection.handle();
    let mut future = Box::pin(inbound.respond(response(true), Duration::from_secs(1)));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(matches!(
        future.as_mut().poll(&mut context),
        std::task::Poll::Ready(Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::InvalidConfiguration { .. },
            ..
        }))
    ));
    assert_eq!(handle.snapshot().response_write_attempted, 0);
    drop(future);
    runtime.block_on(async {
        connection.shutdown().await.unwrap();
        server.await.unwrap();
    });
}

#[tokio::test]
async fn direct_reply_preserves_signed_id_nullable_headers_and_original_branch_identity() {
    for (id, commit) in [(i32::MIN, true), (i32::MAX, false)] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = callback(id, commit);
            write(&mut socket, request.clone()).await;
            let result = read(&mut socket).await;
            assert_eq!(result.id, id);
            assert_eq!(result.message_type, RequestType::Response);
            assert_eq!(result.head_map, request.head_map);
            assert_eq!(result.body, Some(response(commit)));
            eof(&mut socket).await;
        });
        let mut connection = TcpConnection::connect(address, config(2)).await.unwrap();
        let handle = connection.handle();
        let inbound = connection.next_request().await.unwrap();
        assert_eq!(inbound.connection().generation(), handle.generation());
        inbound
            .respond(response(commit), Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(handle.snapshot().pending, 0);
        assert_eq!(handle.snapshot().response_write_attempted, 1);
        assert_eq!(handle.snapshot().responses_written, 1);
        assert_eq!(handle.snapshot().write_attempted, 0);
        connection.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn callback_and_outbound_call_can_share_wire_id_without_cross_completion() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let business = read(&mut socket).await;
        write(&mut socket, callback(business.id, true)).await;
        let result = read(&mut socket).await;
        assert_eq!(result.id, business.id);
        assert_eq!(result.body, Some(response(true)));
        write(
            &mut socket,
            peer::heartbeat(business.id, RequestType::HeartbeatResponse),
        )
        .await;
        write(
            &mut socket,
            reply(business.id, begin_result("separate-future")),
        )
        .await;
        eof(&mut socket).await;
    });
    let mut connection = TcpConnection::connect(address, config(1)).await.unwrap();
    let handle = connection.handle();
    let caller = handle.clone();
    let call = tokio::spawn(async move {
        caller
            .request(begin("outbound"), Duration::from_secs(2))
            .await
    });
    let request = connection.next_request().await.unwrap();
    assert_eq!(handle.snapshot().pending, 1);
    request
        .respond(response(true), Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(peer::xid(call.await.unwrap().unwrap()), "separate-future");
    assert_eq!(handle.snapshot().completed, 1);
    assert_eq!(handle.snapshot().responses_written, 1);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn wrong_family_or_branch_identity_is_rejected_without_writing() {
    for mutation in 0..4 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            write(&mut socket, callback(7, true)).await;
            let valid = read(&mut socket).await;
            assert!(
                matches!(valid.body, Some(Message::GlobalStatusRequest(_))),
                "invalid response must not reach socket"
            );
            write(&mut socket, reply(valid.id, peer::status_result())).await;
            eof(&mut socket).await;
        });
        let mut connection = TcpConnection::connect(address, config(2)).await.unwrap();
        let handle = connection.handle();
        let inbound = connection.next_request().await.unwrap();
        let mut body = response(true);
        match mutation {
            0 => body = response(false),
            1 => {
                if let Message::BranchCommitResponse(value) = &mut body {
                    value.xid = Some("other-xid".into());
                }
            }
            2 => {
                if let Message::BranchCommitResponse(value) = &mut body {
                    value.branch_id += 1;
                }
            }
            _ => {
                if let Message::BranchCommitResponse(value) = &mut body {
                    value.xid = None;
                }
            }
        }
        assert!(matches!(
            inbound.respond(body, Duration::from_secs(1)).await,
            Err(CallError::LocalFailureBeforeSend {
                reason: TransportError::UnexpectedResponse { .. },
                ..
            })
        ));
        assert_eq!(handle.snapshot().response_write_attempted, 0);
        handle
            .request(
                Message::GlobalStatusRequest(seata_protocol::transaction::GlobalStatusRequest {
                    xid: Some("known".into()),
                    extra_data: None,
                }),
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        connection.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn oneway_or_expired_reply_never_writes_an_ordinary_response() {
    for oneway in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut frame = callback(8, true);
            if oneway {
                frame.message_type = RequestType::RequestOneway;
            }
            write(&mut socket, frame).await;
            eof(&mut socket).await;
        });
        let mut connection = TcpConnection::connect(address, config(1)).await.unwrap();
        let handle = connection.handle();
        let inbound = connection.next_request().await.unwrap();
        assert!(matches!(
            inbound
                .respond(
                    response(true),
                    if oneway {
                        Duration::from_secs(1)
                    } else {
                        Duration::ZERO
                    }
                )
                .await,
            Err(CallError::LocalFailureBeforeSend { .. })
        ));
        assert_eq!(handle.snapshot().response_write_attempted, 0);
        connection.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn complete_flat_callback_batch_checks_every_child_identity_before_reply() {
    for invalid in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut frame = reply(
                -7,
                Message::MergedWarpMessage(MergedWarpMessage {
                    msgs: vec![
                        callback(1, true).body.unwrap(),
                        callback(2, false).body.unwrap(),
                    ],
                    msg_ids: vec![1, 2],
                }),
            );
            frame.message_type = RequestType::RequestSync;
            write(&mut socket, frame).await;
            if !invalid {
                let reply = read(&mut socket).await;
                assert_eq!(reply.id, -7);
                assert!(matches!(reply.body, Some(Message::MergeResultMessage(_))));
            }
            eof(&mut socket).await;
        });
        let mut connection = TcpConnection::connect(address, config(1)).await.unwrap();
        let inbound = connection.next_request().await.unwrap();
        let mut last = response(false);
        if invalid {
            if let Message::BranchRollbackResponse(value) = &mut last {
                value.branch_id = 0;
            }
        }
        let result = inbound
            .respond(
                Message::MergeResultMessage(MergeResultMessage {
                    msgs: Some(vec![response(true), last]),
                }),
                Duration::from_secs(1),
            )
            .await;
        if invalid {
            assert!(matches!(
                result,
                Err(CallError::LocalFailureBeforeSend { .. })
            ));
        } else {
            result.unwrap();
        }
        connection.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn old_pool_callback_cannot_reply_over_replacement_generation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (close_tx, close_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        write(&mut first, callback(9, true)).await;
        close_rx.await.unwrap();
        drop(first);
        let (mut second, _) = listener.accept().await.unwrap();
        let business = read(&mut second).await;
        assert!(matches!(
            business.body,
            Some(Message::GlobalBeginRequest(_))
        ));
        write(&mut second, reply(business.id, begin_result("replacement"))).await;
        eof(&mut second).await;
    });
    let settings = ConnectionPoolConfig::new(
        address,
        config(2),
        1,
        2,
        ReconnectPolicy::new(
            Duration::from_millis(20),
            Duration::from_millis(80),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap();
    let mut pool = ConnectionPool::start(settings).unwrap();
    let handle = pool.handle();
    let inbound = pool.next_inbound().await.unwrap();
    let old = inbound.connection().clone();
    close_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), old.closed())
        .await
        .unwrap();
    let new = handle.acquire(Duration::from_secs(2)).await.unwrap();
    assert_ne!(old.generation(), new.generation());
    assert!(matches!(
        inbound
            .respond(response(true), Duration::from_secs(1))
            .await,
        Err(CallError::LocalFailureBeforeSend { .. })
    ));
    assert_eq!(
        peer::xid(
            handle
                .request(begin("new"), Duration::from_secs(1))
                .await
                .unwrap()
        ),
        "replacement"
    );
    pool.shutdown().await.unwrap();
    server.await.unwrap();
}
