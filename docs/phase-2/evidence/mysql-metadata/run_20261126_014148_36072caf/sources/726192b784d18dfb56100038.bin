//! 真实loopback TCP测试；服务端仅作协议与故障注入，不模拟业务持久化/TC事务状态机。
#![allow(clippy::unwrap_used)]
#[path = "support/session_peer.rs"]
mod peer;
use peer::{begin, begin_result, config, eof, heartbeat, read, reply, status_result, write, xid};
use seata_protocol::transaction::GlobalStatusRequest;
use seata_protocol::{Message, RequestType};
use seata_transport::{CallError, TcpConnection, TransportError};
use std::time::Duration;
use tokio::net::{TcpListener, TcpSocket};
use tokio::sync::oneshot;

#[tokio::test]
async fn refused_connection_is_known_local_before_any_request_write() {
    let bound = TcpSocket::new_v4().unwrap();
    bound.bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let result = TcpConnection::connect(bound.local_addr().unwrap(), config(2)).await;
    assert!(matches!(
        result,
        Err(CallError::LocalFailureBeforeSend {
            request_id: None,
            ..
        })
    ));
}
#[tokio::test]
async fn real_request_response_and_same_id_pong_do_not_cross_complete() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read(&mut socket).await;
        assert_eq!(request.message_type, RequestType::RequestSync);
        write(
            &mut socket,
            heartbeat(request.id, RequestType::HeartbeatResponse),
        )
        .await;
        write(
            &mut socket,
            heartbeat(request.id, RequestType::HeartbeatRequest),
        )
        .await;
        let pong = read(&mut socket).await;
        assert_eq!(pong.id, request.id);
        assert_eq!(pong.message_type, RequestType::HeartbeatResponse);
        write(&mut socket, reply(request.id, begin_result("xid-ok"))).await;
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(2)).await.unwrap();
    let handle = connection.handle();
    assert_eq!(
        xid(handle
            .request(begin("first"), Duration::from_secs(2))
            .await
            .unwrap()),
        "xid-ok"
    );
    assert_eq!(handle.snapshot().pending, 0);
    assert_eq!(handle.snapshot().completed, 1);
    assert_eq!(handle.snapshot().heartbeats_received, 2);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn timeout_after_observed_write_is_unknown_and_late_duplicates_cannot_complete_next_call() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (received_tx, received_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let first = read(&mut socket).await;
        received_tx.send(()).unwrap();
        release_rx.await.unwrap();
        write(&mut socket, reply(first.id, begin_result("late"))).await;
        write(&mut socket, reply(first.id, begin_result("duplicate"))).await;
        let second = read(&mut socket).await;
        assert_ne!(first.id, second.id);
        assert!(matches!(second.body, Some(Message::GlobalStatusRequest(_))));
        write(&mut socket, reply(first.id, begin_result("older"))).await;
        write(&mut socket, reply(second.id, status_result())).await;
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(2)).await.unwrap();
    let handle = connection.handle();
    let call_handle = handle.clone();
    let call = tokio::spawn(async move {
        call_handle
            .request(begin("only-once"), Duration::from_millis(150))
            .await
    });
    received_rx.await.unwrap();
    let error = call.await.unwrap().unwrap_err();
    assert!(error.is_unknown_outcome());
    assert_eq!(handle.snapshot().pending, 0);
    assert_eq!(handle.snapshot().timed_out_after_send, 1);
    release_tx.send(()).unwrap();
    let result = handle
        .request(
            Message::GlobalStatusRequest(GlobalStatusRequest {
                xid: Some("separate-existing-tx".into()),
                extra_data: None,
            }),
            Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert!(matches!(
        result.body,
        Some(Message::GlobalStatusResponse(_))
    ));
    assert!(handle.snapshot().unmatched_responses >= 3);
    assert_eq!(handle.snapshot().completed, 1);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn cancellation_removes_waiter_and_late_response_is_only_observed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (received_tx, received_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read(&mut socket).await;
        received_tx.send(()).unwrap();
        release_rx.await.unwrap();
        write(
            &mut socket,
            reply(request.id, begin_result("cancelled-late")),
        )
        .await;
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(1)).await.unwrap();
    let handle = connection.handle();
    let caller = handle.clone();
    let task = tokio::spawn(async move {
        caller
            .request(begin("cancel"), Duration::from_secs(5))
            .await
    });
    received_rx.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(handle.snapshot().pending, 0);
    assert_eq!(handle.snapshot().cancelled_after_send, 1);
    release_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while handle.snapshot().unmatched_responses < 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(handle.snapshot().completed, 0);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn remote_close_after_read_is_unknown_and_cleans_all_waiters() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(1)).await.unwrap();
    let handle = connection.handle();
    assert!(handle
        .request(begin("disconnect"), Duration::from_secs(2))
        .await
        .unwrap_err()
        .is_unknown_outcome());
    assert_eq!(handle.snapshot().pending, 0);
    assert!(handle.snapshot().closed);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn wrong_response_type_is_unknown_protocol_failure_not_definite_rejection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read(&mut socket).await;
        write(&mut socket, reply(request.id, status_result())).await;
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(1)).await.unwrap();
    let handle = connection.handle();
    let error = handle
        .request(begin("type-guard"), Duration::from_secs(2))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        CallError::UnknownOutcome {
            reason: TransportError::UnexpectedResponse { .. },
            ..
        }
    ));
    assert!(handle.snapshot().closed);
    assert_eq!(handle.snapshot().completed, 0);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn concurrent_reverse_responses_preserve_signed_wrap_and_require_quota_rotation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut requests = vec![];
        for _ in 0..3 {
            requests.push(read(&mut socket).await);
        }
        let mut ids: Vec<_> = requests.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![i32::MIN, i32::MAX - 1, i32::MAX]);
        for request in requests.into_iter().rev() {
            let name = match request.body.unwrap() {
                Message::GlobalBeginRequest(v) => v.transaction_name.unwrap(),
                _ => panic!("wrong request"),
            };
            write(
                &mut socket,
                reply(request.id, begin_result(&format!("xid-{name}"))),
            )
            .await;
        }
        eof(&mut socket).await;
    });
    let connection =
        TcpConnection::connect(addr, config(3).with_id_limits(i32::MAX - 1, 3).unwrap())
            .await
            .unwrap();
    let h = connection.handle();
    let (a, b, c) = tokio::join!(
        h.request(begin("a"), Duration::from_secs(2)),
        h.request(begin("b"), Duration::from_secs(2)),
        h.request(begin("c"), Duration::from_secs(2))
    );
    assert_eq!(xid(a.unwrap()), "xid-a");
    assert_eq!(xid(b.unwrap()), "xid-b");
    assert_eq!(xid(c.unwrap()), "xid-c");
    assert!(matches!(
        h.request(begin("no-fourth"), Duration::from_secs(1)).await,
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::RequestIdRotationRequired { .. },
            ..
        })
    ));
    assert_eq!(h.snapshot().pending, 0);
    assert_eq!(h.snapshot().completed, 3);
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn shutdown_resolves_outstanding_requests_and_rejects_new_calls_before_send() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (seen_tx, seen_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read(&mut socket).await;
        read(&mut socket).await;
        seen_tx.send(()).unwrap();
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(2)).await.unwrap();
    let h = connection.handle();
    let a = h.clone();
    let b = h.clone();
    let first = tokio::spawn(async move { a.request(begin("a"), Duration::from_secs(5)).await });
    let second = tokio::spawn(async move { b.request(begin("b"), Duration::from_secs(5)).await });
    seen_rx.await.unwrap();
    connection.shutdown().await.unwrap();
    assert!(first.await.unwrap().unwrap_err().is_unknown_outcome());
    assert!(second.await.unwrap().unwrap_err().is_unknown_outcome());
    assert_eq!(h.snapshot().pending, 0);
    assert!(matches!(
        h.request(begin("closed"), Duration::from_secs(1)).await,
        Err(CallError::LocalFailureBeforeSend { .. })
    ));
    server.await.unwrap();
}
#[tokio::test]
async fn bounded_pending_capacity_rejects_overload_before_write() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (seen_tx, seen_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read(&mut socket).await;
        seen_tx.send(()).unwrap();
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(1)).await.unwrap();
    let h = connection.handle();
    let a = h.clone();
    let first = tokio::spawn(async move { a.request(begin("held"), Duration::from_secs(5)).await });
    seen_rx.await.unwrap();
    assert!(matches!(
        h.request(begin("overloaded"), Duration::from_secs(1)).await,
        Err(CallError::LocalFailureBeforeSend {
            reason: TransportError::Overloaded,
            ..
        })
    ));
    assert_eq!(h.snapshot().pending, 1);
    connection.shutdown().await.unwrap();
    assert!(first.await.unwrap().unwrap_err().is_unknown_outcome());
    server.await.unwrap();
}

#[tokio::test]
async fn two_live_generations_can_share_wire_id_without_cross_completion() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut old, _) = listener.accept().await.unwrap();
        let (mut new, _) = listener.accept().await.unwrap();
        let a = read(&mut old).await;
        let b = read(&mut new).await;
        assert_eq!(a.id, b.id);
        write(&mut new, reply(b.id, begin_result("new-generation"))).await;
        write(&mut old, reply(a.id, begin_result("old-generation"))).await;
        eof(&mut old).await;
        eof(&mut new).await;
    });
    let old = TcpConnection::connect(addr, config(1)).await.unwrap();
    let new = TcpConnection::connect(addr, config(1)).await.unwrap();
    let a = old.handle();
    let b = new.handle();
    assert_ne!(a.generation(), b.generation());
    let (x, y) = tokio::join!(
        a.request(begin("a"), Duration::from_secs(2)),
        b.request(begin("b"), Duration::from_secs(2))
    );
    assert_eq!(xid(x.unwrap()), "old-generation");
    assert_eq!(xid(y.unwrap()), "new-generation");
    old.shutdown().await.unwrap();
    new.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn dropping_owner_closes_socket_and_resolves_started_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (seen_tx, seen_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read(&mut socket).await;
        seen_tx.send(()).unwrap();
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(1)).await.unwrap();
    let handle = connection.handle();
    let caller = handle.clone();
    let call =
        tokio::spawn(async move { caller.request(begin("drop"), Duration::from_secs(5)).await });
    seen_rx.await.unwrap();
    drop(connection);
    assert!(call.await.unwrap().unwrap_err().is_unknown_outcome());
    assert_eq!(handle.snapshot().pending, 0);
    assert!(handle.snapshot().closed);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn explicit_failed_result_preserves_even_contradictory_status_fields() {
    use seata_protocol::{GlobalStatus, ResultCode, TransactionExceptionCode};
    let mut body = status_result();
    if let Message::GlobalStatusResponse(value) = &mut body {
        value.response.result_code = ResultCode::Failed;
        value.response.transaction_exception_code = TransactionExceptionCode::Unknown;
        value.response.msg = Some("explicit TC rejection".into());
        value.global_status = GlobalStatus::Committed;
    }
    let expected = body.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read(&mut socket).await;
        write(&mut socket, reply(request.id, body)).await;
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(1)).await.unwrap();
    let result = connection
        .handle()
        .request(
            Message::GlobalStatusRequest(GlobalStatusRequest {
                xid: Some("known".into()),
                extra_data: None,
            }),
            Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert_eq!(result.body, Some(expected));
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn flat_batch_requires_all_response_types_and_child_count_before_completion() {
    use seata_protocol::protocol::{MergeResultMessage, MergedWarpMessage};
    // 成功、末个类型错误、子项缺失三种形状，禁止只比较第一个结果就完成整批。
    for shape in 0..3 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read(&mut socket).await;
            let mut children = vec![begin_result("batch"), status_result()];
            if shape == 1 {
                children[1] = begin_result("wrong-tail");
            } else if shape == 2 {
                children.pop();
            }
            write(
                &mut socket,
                reply(
                    request.id,
                    Message::MergeResultMessage(MergeResultMessage {
                        msgs: Some(children),
                    }),
                ),
            )
            .await;
            eof(&mut socket).await;
        });
        let connection = TcpConnection::connect(addr, config(1)).await.unwrap();
        let handle = connection.handle();
        let body = Message::MergedWarpMessage(MergedWarpMessage {
            msgs: vec![
                begin("batch"),
                Message::GlobalStatusRequest(GlobalStatusRequest {
                    xid: Some("known".into()),
                    extra_data: None,
                }),
            ],
            msg_ids: vec![],
        });
        let result = handle.request(body, Duration::from_secs(2)).await;
        if shape == 0 {
            assert!(matches!(
                result.unwrap().body,
                Some(Message::MergeResultMessage(_))
            ));
            assert_eq!(handle.snapshot().completed, 1);
        } else {
            assert!(matches!(
                result,
                Err(CallError::UnknownOutcome {
                    reason: TransportError::UnexpectedResponse { .. },
                    ..
                })
            ));
            assert_eq!(handle.snapshot().completed, 0);
            assert!(handle.snapshot().closed);
        }
        assert_eq!(handle.snapshot().pending, 0);
        connection.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn local_validation_writes_nothing_and_keeps_connection_usable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read(&mut socket).await;
        assert!(
            matches!(request.body,Some(Message::GlobalBeginRequest(ref value)) if value.transaction_name.as_deref()==Some("valid"))
        );
        write(&mut socket, reply(request.id, begin_result("valid"))).await;
        eof(&mut socket).await;
    });
    let connection = TcpConnection::connect(addr, config(1)).await.unwrap();
    let handle = connection.handle();
    for (body, timeout) in [
        (begin("zero-timeout"), Duration::ZERO),
        (status_result(), Duration::from_secs(1)),
        (begin(&"x".repeat(70000)), Duration::from_secs(1)),
    ] {
        assert!(matches!(
            handle.request(body, timeout).await,
            Err(CallError::LocalFailureBeforeSend { .. })
        ));
        assert_eq!(handle.snapshot().pending, 0);
        assert_eq!(handle.snapshot().write_attempted, 0);
    }
    assert_eq!(
        xid(handle
            .request(begin("valid"), Duration::from_secs(2))
            .await
            .unwrap()),
        "valid"
    );
    connection.shutdown().await.unwrap();
    server.await.unwrap();
}
