//! 正式TM业务作用域/六传播策略；真实TCP断言全局结束只能由Launcher发送。
#![allow(clippy::unwrap_used)]
#[path = "support/tm_peer.rs"]
mod fixture;
use fixture::{begin, config, end_response, manager, peer, registered};
use seata_protocol::{GlobalStatus, Message};
use seata_tm::{
    GlobalTransactionRole, Propagation, RootContext, TransactionCompletion, TransactionConfig,
    TransactionContext, TransactionError, Xid,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

async fn end(socket: &mut tokio::net::TcpStream, commit: bool, xid: &str, failed: bool) {
    let request = peer::read(socket).await;
    match request.body {
        Some(Message::GlobalCommitRequest(value)) if commit => {
            assert_eq!(value.xid.as_deref(), Some(xid))
        }
        Some(Message::GlobalRollbackRequest(value)) if !commit => {
            assert_eq!(value.xid.as_deref(), Some(xid))
        }
        other => panic!("unexpected end: {other:?}"),
    }
    let mut response = peer::success();
    if failed {
        response.result_code = seata_protocol::ResultCode::Failed;
        response.msg = Some("rollback diagnostic".into());
    }
    peer::write(
        socket,
        peer::reply(
            request.id,
            end_response(
                commit,
                if commit {
                    GlobalStatus::Committed
                } else if failed {
                    GlobalStatus::RollbackRetrying
                } else {
                    GlobalStatus::Rollbacked
                },
                response,
            ),
        ),
    )
    .await;
}

#[tokio::test]
async fn required_nested_scope_participates_without_rewriting_outer_launcher_or_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("outer")).await;
        end(&mut socket, true, "outer", false).await;
        peer::eof(&mut socket).await;
    });
    let result = tm
        .execute(config(), || {
            assert_eq!(
                RootContext::current().unwrap().role(),
                GlobalTransactionRole::Launcher
            );
            async {
                let outer = RootContext::current().unwrap();
                let nested = tm
                    .execute(config(), || async {
                        let ctx = RootContext::current().unwrap();
                        assert_eq!(ctx.role(), GlobalTransactionRole::Participant);
                        assert_eq!(ctx.xid(), outer.xid());
                        assert_eq!(ctx.deadline(), outer.deadline());
                        Ok::<_, &str>(41)
                    })
                    .await
                    .unwrap();
                assert_eq!(*nested.value(), 41);
                assert!(matches!(
                    nested.completion(),
                    TransactionCompletion::Participant(_)
                ));
                assert_eq!(RootContext::current().unwrap(), outer);
                Ok::<_, &str>(42)
            }
        })
        .await
        .unwrap();
    assert_eq!(*result.value(), 42);
    assert!(matches!(
        result.completion(),
        TransactionCompletion::Observed { .. }
    ));
    assert!(RootContext::current().is_none());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn requires_new_suspends_outer_and_not_supported_clears_then_restores_context() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("outer")).await;
        begin(&mut socket, Some("inner")).await;
        end(&mut socket, true, "inner", false).await;
        end(&mut socket, true, "outer", false).await;
        peer::eof(&mut socket).await;
    });
    tm.execute(config(), || async {
        let outer = RootContext::current().unwrap();
        let inner = TransactionConfig::builder("business".into())
            .transaction_timeout(Duration::from_secs(30))
            .propagation(Propagation::RequiresNew)
            .build()
            .unwrap();
        tm.execute(inner, || async {
            assert_eq!(RootContext::current().unwrap().xid().as_str(), "inner");
            assert_eq!(
                RootContext::current().unwrap().role(),
                GlobalTransactionRole::Launcher
            );
            Ok::<_, &str>(())
        })
        .await
        .unwrap();
        let unmanaged = TransactionConfig::builder("business".into())
            .propagation(Propagation::NotSupported)
            .build()
            .unwrap();
        let value = tm
            .execute(unmanaged, || async {
                assert!(RootContext::current().is_none());
                Ok::<_, &str>(())
            })
            .await
            .unwrap();
        assert_eq!(value.completion(), &TransactionCompletion::Unmanaged);
        assert_eq!(RootContext::current().unwrap(), outer);
        Ok::<_, &str>(())
    })
    .await
    .unwrap();
    assert!(RootContext::current().is_none());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn mandatory_never_and_supports_observe_context_before_business_or_network() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let (registered_tx, registered_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        registered_tx.send(()).unwrap();
        peer::eof(&mut socket).await;
    });
    let mandatory = TransactionConfig::builder("business".into())
        .propagation(Propagation::Mandatory)
        .build()
        .unwrap();
    assert!(tm
        .execute(mandatory.clone(), || async {
            panic!("invalid Mandatory must not construct business");
            #[allow(unreachable_code)]
            Ok::<_, &str>(())
        })
        .await
        .is_err());
    for policy in [
        Propagation::Supports,
        Propagation::Never,
        Propagation::NotSupported,
    ] {
        let cfg = TransactionConfig::builder("business".into())
            .propagation(policy)
            .build()
            .unwrap();
        let value = tm
            .execute(cfg, || async {
                assert!(RootContext::current().is_none());
                Ok::<_, &str>(())
            })
            .await
            .unwrap();
        assert_eq!(value.completion(), &TransactionCompletion::Unmanaged);
    }
    RootContext::scope(
        Some(TransactionContext::participant(
            Xid::parse("incoming".into()).unwrap(),
        )),
        async {
            for policy in [
                Propagation::Required,
                Propagation::Supports,
                Propagation::Mandatory,
            ] {
                let cfg = TransactionConfig::builder("business".into())
                    .propagation(policy)
                    .build()
                    .unwrap();
                let value = tm
                    .execute(cfg, || async {
                        assert_eq!(
                            RootContext::current().unwrap().role(),
                            GlobalTransactionRole::Participant
                        );
                        Ok::<_, &str>(())
                    })
                    .await
                    .unwrap();
                assert!(matches!(
                    value.completion(),
                    TransactionCompletion::Participant(_)
                ));
            }
            let never = TransactionConfig::builder("business".into())
                .propagation(Propagation::Never)
                .build()
                .unwrap();
            assert!(tm
                .execute(never, || async {
                    panic!("invalid Never must not run business");
                    #[allow(unreachable_code)]
                    Ok::<_, &str>(())
                })
                .await
                .is_err());
        },
    )
    .await;
    // 等待测试端真实写完登记ACK；固定sleep不能保证分片写入已完成。
    tokio::time::timeout(Duration::from_secs(3), registered_rx)
        .await
        .unwrap()
        .unwrap();
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn business_error_and_rollback_rejection_are_both_returned_with_original_xid() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("failed-business")).await;
        end(&mut socket, false, "failed-business", true).await;
        peer::eof(&mut socket).await;
    });
    let error = tm
        .execute(config(), || async { Err::<(), _>("business error") })
        .await
        .unwrap_err();
    match error {
        TransactionError::Business {
            error,
            xid,
            rollback,
        } => {
            assert_eq!(error, "business error");
            assert_eq!(xid.unwrap().as_str(), "failed-business");
            assert!(rollback.unwrap().is_err());
        }
        other => panic!("{other:?}"),
    }
    assert!(RootContext::current().is_none());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn expired_business_is_cancelled_and_rolls_back_once_without_commit() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(1));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        match request.body {
            Some(Message::GlobalBeginRequest(value)) => assert_eq!(value.timeout, 30),
            other => panic!("{other:?}"),
        }
        peer::write(
            &mut socket,
            peer::reply(request.id, peer::begin_result("deadline")),
        )
        .await;
        end(&mut socket, false, "deadline", false).await;
        peer::eof(&mut socket).await;
    });
    let cfg = TransactionConfig::builder("business".into())
        .transaction_timeout(Duration::from_millis(30))
        .build()
        .unwrap();
    let error = tm
        .execute(cfg, || async {
            std::future::pending::<Result<(), &str>>().await
        })
        .await
        .unwrap_err();
    match error {
        TransactionError::Deadline { xid, rollback } => {
            assert_eq!(xid.as_str(), "deadline");
            assert!(rollback.is_ok());
        }
        other => panic!("{other:?}"),
    }
    assert!(RootContext::current().is_none());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn lost_scoped_commit_never_falls_through_to_rollback_or_background_retry() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_millis(100));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("unknown")).await;
        assert!(matches!(
            peer::read(&mut socket).await.body,
            Some(Message::GlobalCommitRequest(_))
        ));
        peer::eof(&mut socket).await;
    });
    let error = tm
        .execute(config(), || async { Ok::<_, &str>(7) })
        .await
        .unwrap_err();
    match error {
        TransactionError::Tm(error) => {
            assert!(error.is_unknown_outcome());
            assert_eq!(error.xid().unwrap().as_str(), "unknown");
        }
        other => panic!("{other:?}"),
    }
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn panic_and_cancel_restore_context_without_unowned_network_cleanup() {
    for panic in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tm = Arc::new(manager(
            listener.local_addr().unwrap(),
            Duration::from_secs(1),
        ));
        let server = tokio::spawn(async move {
            let mut socket = registered(&listener).await;
            begin(&mut socket, Some("abandoned")).await;
            peer::eof(&mut socket).await;
        });
        let (ready, seen) = tokio::sync::oneshot::channel();
        let cloned = Arc::clone(&tm);
        let task = tokio::spawn(async move {
            cloned
                .execute(config(), || async {
                    ready
                        .send(RootContext::current().unwrap().xid().clone())
                        .unwrap();
                    if panic {
                        panic!("business panic");
                    }
                    std::future::pending::<Result<(), &str>>().await
                })
                .await
        });
        assert_eq!(seen.await.unwrap().as_str(), "abandoned");
        if panic {
            assert!(task.await.unwrap_err().is_panic());
        } else {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        }
        assert!(RootContext::current().is_none());
        Arc::try_unwrap(tm).unwrap().shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn unknown_scoped_end_can_recover_original_intent_without_starting_a_new_transaction() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_millis(100));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("recover-original")).await;
        assert!(matches!(
            peer::read(&mut socket).await.body,
            Some(Message::GlobalCommitRequest(_))
        ));
        let query = peer::read(&mut socket).await;
        match query.body {
            Some(Message::GlobalStatusRequest(value)) => {
                assert_eq!(value.xid.as_deref(), Some("recover-original"))
            }
            other => panic!("recovery sent a new Begin or wrong operation: {other:?}"),
        }
        peer::write(
            &mut socket,
            peer::reply(
                query.id,
                Message::GlobalStatusResponse(seata_protocol::transaction::GlobalStatusResponse {
                    response: peer::success(),
                    global_status: GlobalStatus::Begin,
                }),
            ),
        )
        .await;
        end(&mut socket, true, "recover-original", false).await;
        peer::eof(&mut socket).await;
    });
    let error = match tm
        .execute(config(), || async { Ok::<_, &str>(()) })
        .await
        .unwrap_err()
    {
        TransactionError::Tm(error) => error,
        other => panic!("{other:?}"),
    };
    for operation in [
        seata_tm::TmOperation::Begin,
        seata_tm::TmOperation::Status,
        seata_tm::TmOperation::Rollback,
    ] {
        let mut invalid = error.as_ref().clone();
        if let seata_tm::TmError::UnknownOutcome {
            operation: original,
            ..
        } = &mut invalid
        {
            *original = operation;
        }
        assert!(tm.recover_unknown(&invalid).is_err());
    }
    let mut changed_xid = error.as_ref().clone();
    if let seata_tm::TmError::UnknownOutcome { xid, .. } = &mut changed_xid {
        *xid = Some(Xid::parse("another-xid".into()).unwrap());
    }
    assert!(tm.recover_unknown(&changed_xid).is_err());
    let unavailable = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let other_manager = manager(
        unavailable.local_addr().unwrap(),
        Duration::from_millis(100),
    );
    assert!(other_manager.recover_unknown(&error).is_err());
    other_manager.shutdown().await.unwrap();
    let mut recovered = tm.recover_unknown(&error).unwrap();
    assert_eq!(recovered.xid().as_str(), "recover-original");
    assert!(recovered.has_unknown_outcome());
    assert!(recovered.commit().await.is_err());
    assert!(recovered.rollback().await.is_err());
    assert_eq!(
        recovered.reconcile().await.unwrap().status(),
        GlobalStatus::Begin
    );
    assert!(recovered.rollback().await.is_err());
    assert!(matches!(
        recovered.commit().await.unwrap(),
        TransactionCompletion::Observed { .. }
    ));
    assert!(!recovered.has_unknown_outcome());
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}
