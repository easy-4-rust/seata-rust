//! 通过真实TCP观察正式TM的登记、请求身份、明确TC响应及未知结果处理。
#![allow(clippy::unwrap_used)]
use seata_protocol::transaction::{AbstractTransactionResponse, GlobalStatusResponse};
use seata_protocol::{GlobalStatus, Message, ResultCode, TransactionExceptionCode};
use seata_tm::{
    GlobalTransactionRole, RootContext, TmError, TmOperation, TransactionCompletion, Xid,
};
use std::time::Duration;
use tokio::net::TcpListener;

#[path = "support/tm_peer.rs"]
mod fixture;
use fixture::{begin, config, end_response, manager, peer, registered};

#[tokio::test]
async fn native_tm_begin_binds_only_lexical_scope_and_end_preserves_exact_xid() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:9007199254740993")).await;
        let request = peer::read(&mut socket).await;
        match request.body {
            Some(Message::GlobalCommitRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("tc:9007199254740993"))
            }
            other => panic!("{other:?}"),
        }
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                end_response(true, GlobalStatus::AsyncCommitting, peer::success()),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let mut tx = tm.begin(&config()).await.unwrap();
    let observed = tm.connection_snapshot();
    assert_eq!(observed.ready(), 1);
    let current = observed.slots[0].connection.as_ref().unwrap();
    assert_eq!(Some(current.generation), observed.slots[0].generation);
    assert_eq!(current.pending, 0);
    assert!(current.completed >= 1);
    assert!(tm.connection_sampler().snapshot().cumulative.completed >= 1);
    assert_eq!(tx.xid().as_str(), "tc:9007199254740993");
    assert_eq!(tx.extra_data(), Some("server-extra"));
    assert!(!format!("{tx:?}").contains("server-extra"));
    assert!(RootContext::current().is_none());
    tx.scope(|| async {
        let ctx = RootContext::current().unwrap();
        assert_eq!(ctx.xid(), tx.xid());
        assert_eq!(ctx.role(), GlobalTransactionRole::Launcher);
        assert!(ctx.deadline().is_some());
        assert_eq!(
            RootContext::capture().unwrap().role(),
            GlobalTransactionRole::Participant
        );
    })
    .await;
    assert!(RootContext::current().is_none());
    match tx.commit().await.unwrap() {
        TransactionCompletion::Observed { xid, outcome } => {
            assert_eq!(xid, tx.xid().clone());
            assert_eq!(outcome.status(), GlobalStatus::AsyncCommitting);
            assert_eq!(outcome.response().result_code, ResultCode::Success);
        }
        other => panic!("{other:?}"),
    }
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn failed_commit_preserves_exception_optional_message_and_authoritative_status() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:1")).await;
        let request = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                end_response(
                    true,
                    GlobalStatus::CommitRetryTimeout,
                    AbstractTransactionResponse {
                        result_code: ResultCode::Failed,
                        msg: Some("kept-message".into()),
                        transaction_exception_code: TransactionExceptionCode::BeginFailed,
                    },
                ),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let mut tx = tm.begin(&config()).await.unwrap();
    let error = tx.commit().await.unwrap_err();
    match error {
        TmError::DefiniteRejected {
            operation,
            xid,
            response,
            status,
            ..
        } => {
            assert_eq!(operation, TmOperation::Commit);
            assert_eq!(xid.as_ref(), Some(tx.xid()));
            assert_eq!(response.msg.as_deref(), Some("kept-message"));
            assert_eq!(
                response.transaction_exception_code,
                TransactionExceptionCode::BeginFailed
            );
            assert_eq!(status, Some(GlobalStatus::CommitRetryTimeout));
        }
        other => panic!("{other:?}"),
    }
    assert!(tx.commit().await.is_err());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn successful_begin_without_xid_is_protocol_uncertainty_never_a_usable_launcher() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, None).await;
        peer::eof(&mut socket).await;
    });
    let error = tm.begin(&config()).await.unwrap_err();
    assert!(matches!(
        error,
        TmError::ProtocolFailure {
            operation: TmOperation::Begin,
            may_have_effect: true,
            ..
        }
    ));
    assert!(error.is_unknown_outcome());
    assert!(error.xid().is_none());
    assert!(RootContext::current().is_none());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn lost_begin_response_keeps_unknown_without_returned_xid_or_second_begin() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_millis(80));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        assert!(matches!(request.body, Some(Message::GlobalBeginRequest(_))));
        peer::eof(&mut socket).await;
    });
    let error = tm.begin(&config()).await.unwrap_err();
    assert!(matches!(
        error,
        TmError::UnknownOutcome {
            operation: TmOperation::Begin,
            ..
        }
    ));
    assert!(error.xid().is_none());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn lost_commit_requires_same_xid_status_before_explicit_retry_and_finished_stays_unknown() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_millis(100));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:1")).await;
        let first = peer::read(&mut socket).await;
        assert!(matches!(first.body, Some(Message::GlobalCommitRequest(_))));
        let query = peer::read(&mut socket).await;
        match query.body {
            Some(Message::GlobalStatusRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("tc:1"))
            }
            other => panic!("automatic replay/rollback: {other:?}"),
        }
        peer::write(
            &mut socket,
            peer::reply(
                query.id,
                Message::GlobalStatusResponse(GlobalStatusResponse {
                    response: peer::success(),
                    global_status: GlobalStatus::Finished,
                }),
            ),
        )
        .await;
        let retry = peer::read(&mut socket).await;
        assert_ne!(retry.id, first.id);
        match retry.body {
            Some(Message::GlobalCommitRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("tc:1"))
            }
            other => panic!("{other:?}"),
        }
        peer::write(
            &mut socket,
            peer::reply(
                retry.id,
                end_response(true, GlobalStatus::Finished, peer::success()),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let mut tx = tm.begin(&config()).await.unwrap();
    let error = tx.commit().await.unwrap_err();
    assert_eq!(error.xid(), Some(tx.xid()));
    assert!(error.is_unknown_outcome());
    assert!(tx.has_unknown_outcome());
    assert!(tx.rollback().await.is_err());
    assert!(tx.commit().await.is_err());
    assert_eq!(
        tx.reconcile().await.unwrap().status(),
        GlobalStatus::Finished
    );
    assert!(tx.has_unknown_outcome());
    assert!(matches!(
        tx.commit().await.unwrap(),
        TransactionCompletion::Observed { .. }
    ));
    assert!(tx.has_unknown_outcome());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn participant_never_sends_global_begin_commit_or_rollback() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        peer::eof(&mut socket).await;
    });
    let mut participant = tm.participant(Xid::parse("incoming".into()).unwrap());
    assert_eq!(participant.role(), GlobalTransactionRole::Participant);
    for completion in [
        participant.commit().await.unwrap(),
        participant.rollback().await.unwrap(),
    ] {
        assert!(matches!(completion, TransactionCompletion::Participant(_)));
    }
    tokio::time::sleep(Duration::from_millis(30)).await;
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn malformed_end_response_is_protocol_failure_with_retained_xid_and_possible_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:1")).await;
        let request = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                end_response(false, GlobalStatus::Rollbacked, peer::success()),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let mut tx = tm.begin(&config()).await.unwrap();
    let error = tx.commit().await.unwrap_err();
    assert_eq!(error.xid(), Some(tx.xid()));
    assert!(matches!(
        error,
        TmError::ProtocolFailure {
            may_have_effect: true,
            ..
        }
    ));
    assert!(tx.has_unknown_outcome());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn connection_unavailable_is_local_pre_send_not_tc_rejection_or_unknown() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let tm = manager(address, Duration::from_millis(50));
    let error = tm.begin(&config()).await.unwrap_err();
    assert!(matches!(
        error,
        TmError::LocalFailureBeforeSend {
            operation: TmOperation::Begin,
            ..
        }
    ));
    assert!(!error.is_unknown_outcome());
    tm.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancelled_end_keeps_original_operation_until_authoritative_reconciliation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let (sent, seen) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:1")).await;
        assert!(matches!(
            peer::read(&mut socket).await.body,
            Some(Message::GlobalCommitRequest(_))
        ));
        sent.send(()).unwrap();
        let query = peer::read(&mut socket).await;
        assert!(matches!(query.body, Some(Message::GlobalStatusRequest(_))));
        peer::write(
            &mut socket,
            peer::reply(
                query.id,
                Message::GlobalStatusResponse(GlobalStatusResponse {
                    response: peer::success(),
                    global_status: GlobalStatus::Committed,
                }),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let mut tx = tm.begin(&config()).await.unwrap();
    {
        let call = tx.commit();
        tokio::pin!(call);
        tokio::select! {_=&mut call=>panic!("unexpected Commit result"),_ = seen=>{}}
    }
    assert!(tx.has_unknown_outcome());
    assert!(tx.rollback().await.is_err());
    assert_eq!(
        tx.reconcile().await.unwrap().status(),
        GlobalStatus::Committed
    );
    assert!(!tx.has_unknown_outcome());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn failed_duplicate_with_finished_does_not_erase_original_commit_uncertainty() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_millis(80));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:1")).await;
        assert!(matches!(
            peer::read(&mut socket).await.body,
            Some(Message::GlobalCommitRequest(_))
        ));
        let query = peer::read(&mut socket).await;
        assert!(matches!(query.body, Some(Message::GlobalStatusRequest(_))));
        peer::write(
            &mut socket,
            peer::reply(
                query.id,
                Message::GlobalStatusResponse(GlobalStatusResponse {
                    response: peer::success(),
                    global_status: GlobalStatus::Finished,
                }),
            ),
        )
        .await;
        let retry = peer::read(&mut socket).await;
        assert!(matches!(retry.body, Some(Message::GlobalCommitRequest(_))));
        let mut failed = peer::success();
        failed.result_code = ResultCode::Failed;
        peer::write(
            &mut socket,
            peer::reply(retry.id, end_response(true, GlobalStatus::Finished, failed)),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let mut tx = tm.begin(&config()).await.unwrap();
    assert!(tx.commit().await.unwrap_err().is_unknown_outcome());
    tx.reconcile().await.unwrap();
    assert!(matches!(
        tx.commit().await.unwrap_err(),
        TmError::DefiniteRejected {
            status: Some(GlobalStatus::Finished),
            ..
        }
    ));
    assert!(
        tx.has_unknown_outcome(),
        "Failed/Finished erased the original unanswered Commit"
    );
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn failed_begin_preserves_reported_optional_fields_without_binding_a_launcher() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        for extra in [None, Some("reported-extra".to_owned())] {
            let request = peer::read(&mut socket).await;
            assert!(matches!(request.body, Some(Message::GlobalBeginRequest(_))));
            let mut response = peer::success();
            response.result_code = ResultCode::Failed;
            response.msg = Some("begin rejected".into());
            peer::write(
                &mut socket,
                peer::reply(
                    request.id,
                    Message::GlobalBeginResponse(
                        seata_protocol::transaction::GlobalBeginResponse {
                            response,
                            xid: Some("reported-only".into()),
                            extra_data: extra,
                        },
                    ),
                ),
            )
            .await;
        }
        peer::eof(&mut socket).await;
    });
    for extra in [None, Some("reported-extra")] {
        let error = tm.begin(&config()).await.unwrap_err();
        assert!(error.xid().is_none());
        let original = error.reported_begin().unwrap();
        assert_eq!(original.xid.as_deref(), Some("reported-only"));
        assert_eq!(original.extra_data.as_deref(), extra);
        assert_eq!(original.response.msg.as_deref(), Some("begin rejected"));
        assert!(RootContext::current().is_none());
    }
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn failed_status_retry_timeout_revokes_prior_same_operation_retry_permission() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_millis(80));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:1")).await;
        assert!(matches!(
            peer::read(&mut socket).await.body,
            Some(Message::GlobalCommitRequest(_))
        ));
        for failed in [false, true] {
            let query = peer::read(&mut socket).await;
            assert!(matches!(query.body, Some(Message::GlobalStatusRequest(_))));
            let mut response = peer::success();
            if failed {
                response.result_code = ResultCode::Failed;
                response.msg = Some("retry timeout".into());
            }
            peer::write(
                &mut socket,
                peer::reply(
                    query.id,
                    Message::GlobalStatusResponse(GlobalStatusResponse {
                        response,
                        global_status: if failed {
                            GlobalStatus::CommitRetryTimeout
                        } else {
                            GlobalStatus::Committing
                        },
                    }),
                ),
            )
            .await;
        }
        peer::eof(&mut socket).await;
    });
    let mut tx = tm.begin(&config()).await.unwrap();
    assert!(tx.commit().await.unwrap_err().is_unknown_outcome());
    tx.reconcile().await.unwrap();
    assert!(matches!(
        tx.reconcile().await.unwrap_err(),
        TmError::DefiniteRejected {
            status: Some(GlobalStatus::CommitRetryTimeout),
            ..
        }
    ));
    assert!(
        matches!(tx.commit().await.unwrap_err(), TmError::IllegalState { .. }),
        "Failed status cannot leave the previous retry permission active"
    );
    assert!(tx.has_unknown_outcome());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn fabricated_protocol_diagnostic_cannot_mint_a_launcher_recovery_capability() {
    let unavailable = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let tm = manager(
        unavailable.local_addr().unwrap(),
        Duration::from_millis(100),
    );
    let fabricated = TmError::ProtocolFailure {
        operation: TmOperation::Commit,
        xid: Some(Xid::parse("external-participant".into()).unwrap()),
        may_have_effect: true,
        reason: "fabricated diagnostic",
        source: None,
        recovery: None,
    };
    assert!(tm.recover_unknown(&fabricated).is_err());
    tm.shutdown().await.unwrap();
}
