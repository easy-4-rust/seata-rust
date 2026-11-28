//! 正式 TM 调用的结果计数，不把响应当作全局事务最终完成。
#![allow(clippy::unwrap_used)]
#[path = "support/tm_peer.rs"]
mod fixture;
use fixture::{begin, config, end_response, manager, peer, registered};
use seata_protocol::{GlobalStatus, Message, ResultCode};
use seata_tm::{TmError, TmOperation, Xid};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[tokio::test]
async fn status_counts_authoritative_rejection_unknown_and_stays_readable_after_close() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let metrics = tm.metrics_sampler();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        peer::write(&mut socket, peer::reply(request.id, peer::status_result())).await;
        let request = peer::read(&mut socket).await;
        let mut response = peer::success();
        response.result_code = ResultCode::Failed;
        response.msg = Some("credential-sentinel".into());
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                Message::GlobalStatusResponse(seata_protocol::transaction::GlobalStatusResponse {
                    response,
                    global_status: GlobalStatus::Rollbacking,
                }),
            ),
        )
        .await;
        let request = peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(Message::GlobalStatusRequest(_))
        ));
        drop(socket);
    });
    let xid = Xid::parse("tc:metrics-secret-identity".into()).unwrap();
    tm.get_status(&xid).await.unwrap();
    assert!(matches!(
        tm.get_status(&xid).await,
        Err(TmError::DefiniteRejected { .. })
    ));
    assert!(tm.get_status(&xid).await.unwrap_err().is_unknown_outcome());
    let counts = metrics.snapshot(TmOperation::Status);
    assert_eq!(
        (
            counts.active,
            counts.observed,
            counts.tc_rejected,
            counts.unknown
        ),
        (0, 1, 1, 1)
    );
    assert_eq!(
        (counts.not_sent, counts.local_rejected, counts.cancelled),
        (0, 0, 0)
    );
    assert_eq!(metrics.snapshot(TmOperation::Begin).observed, 0);
    tm.shutdown().await.unwrap();
    assert_eq!(metrics.snapshot(TmOperation::Status), counts);
    server.await.unwrap();
}

#[tokio::test]
async fn begin_and_end_observed_counters_preserve_nonfinal_status_and_no_replay() {
    for commit in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
        let metrics = tm.metrics_sampler();
        let server = tokio::spawn(async move {
            let mut socket = registered(&listener).await;
            begin(&mut socket, Some("tc:metrics-end")).await;
            let request = peer::read(&mut socket).await;
            assert!(matches!(
                (commit, request.body),
                (true, Some(Message::GlobalCommitRequest(_)))
                    | (false, Some(Message::GlobalRollbackRequest(_)))
            ));
            peer::write(
                &mut socket,
                peer::reply(
                    request.id,
                    end_response(
                        commit,
                        if commit {
                            GlobalStatus::AsyncCommitting
                        } else {
                            GlobalStatus::Rollbacking
                        },
                        peer::success(),
                    ),
                ),
            )
            .await;
            peer::eof(&mut socket).await;
        });
        let mut tx = tm.begin(&config()).await.unwrap();
        tx.scope(|| async {
            assert!(matches!(
                tm.begin(&config()).await,
                Err(TmError::IllegalState { .. })
            ));
        })
        .await;
        if commit {
            tx.commit().await.unwrap();
        } else {
            tx.rollback().await.unwrap();
        }
        drop(tx);
        let op = if commit {
            TmOperation::Commit
        } else {
            TmOperation::Rollback
        };
        assert_eq!(metrics.snapshot(TmOperation::Begin).observed, 1);
        assert_eq!(metrics.snapshot(TmOperation::Begin).local_rejected, 1);
        assert_eq!(metrics.snapshot(op).observed, 1);
        assert_eq!(metrics.snapshot(op).unknown, 0);
        assert_eq!(metrics.snapshot(op).active, 0);
        tm.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn cancelled_wait_returns_active_to_zero_without_inventing_a_tc_result() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let metrics = tm.metrics_sampler();
    let (seen_tx, seen) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let _ = peer::read(&mut socket).await;
        seen_tx.send(()).unwrap();
        peer::eof(&mut socket).await;
    });
    let xid = Xid::parse("tc:metrics-cancel".into()).unwrap();
    let mut call = Box::pin(tm.get_status(&xid));
    tokio::select! {
        result = &mut call => panic!("unexpected completion: {result:?}"),
        _ = seen => {}
    }
    assert_eq!(metrics.snapshot(TmOperation::Status).active, 1);
    drop(call);
    let counts = metrics.snapshot(TmOperation::Status);
    assert_eq!(
        (
            counts.active,
            counts.cancelled,
            counts.observed,
            counts.unknown
        ),
        (0, 1, 0, 0)
    );
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn unreachable_peer_is_not_sent_and_invalid_begin_is_protocol_unknown() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    drop(listener);
    let tm = manager(endpoint, Duration::from_millis(30));
    let metrics = tm.metrics_sampler();
    let error = tm.begin(&config()).await.unwrap_err();
    assert!(!error.is_unknown_outcome());
    assert_eq!(metrics.snapshot(TmOperation::Begin).not_sent, 1);
    tm.shutdown().await.unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let metrics = tm.metrics_sampler();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, None).await;
        peer::eof(&mut socket).await;
    });
    let error = tm.begin(&config()).await.unwrap_err();
    assert!(error.is_unknown_outcome());
    let counts = metrics.snapshot(TmOperation::Begin);
    assert_eq!(
        (counts.observed, counts.unknown, counts.protocol_errors),
        (0, 1, 1)
    );
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn lost_end_response_keeps_original_unknown_without_replay_or_opposite_end() {
    for commit in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tm = manager(listener.local_addr().unwrap(), Duration::from_millis(150));
        let metrics = tm.metrics_sampler();
        let server = tokio::spawn(async move {
            let mut socket = registered(&listener).await;
            begin(&mut socket, Some("tc:metrics-lost-end")).await;
            let request = peer::read(&mut socket).await;
            assert!(matches!(
                (commit, request.body),
                (true, Some(Message::GlobalCommitRequest(_)))
                    | (false, Some(Message::GlobalRollbackRequest(_)))
            ));
            // 保持原连接而不回响应；下一次读取必须为关闭，不能有重放或反向结束。
            peer::eof(&mut socket).await;
        });
        let mut tx = tm.begin(&config()).await.unwrap();
        let result = if commit {
            tx.commit().await
        } else {
            tx.rollback().await
        };
        assert!(result.unwrap_err().is_unknown_outcome());
        drop(tx);
        let op = if commit {
            TmOperation::Commit
        } else {
            TmOperation::Rollback
        };
        let opposite = if commit {
            TmOperation::Rollback
        } else {
            TmOperation::Commit
        };
        assert_eq!(metrics.snapshot(op).unknown, 1);
        assert_eq!(metrics.snapshot(op).observed, 0);
        assert_eq!(metrics.snapshot(opposite), Default::default());
        tm.shutdown().await.unwrap();
        server.await.unwrap();
    }
}
