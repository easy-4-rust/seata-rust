//! 原登记错误/迟到响应和原 Try Future 取消的真实数据库边界，不补造原回执。
#![allow(clippy::unwrap_used)]
use super::{command_proxy::XaCommandProxy, fixture, manager, peer};
use seata_db_mysql::{MySqlDataSource, MySqlTccResourceManager, MySqlTccTryError};
use seata_protocol::{transaction::BranchRegisterRequest, BranchType, ResultCode};
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResource, RmCallError};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned MySQL and original V1 registration failure injection"]
async fn original_registration_rejection_loss_and_cancelled_reply_never_start_try() {
    let mut cases = Vec::new();
    for (index, mode) in ["rejected", "lost", "cancelled"].into_iter().enumerate() {
        let (mut admin, source, adapter) = fixture::seed().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, store) =
            manager(adapter.clone(), &listener, &format!("registration-{mode}")).await;
        let xid = format!("tcc:registration:{mode}");
        let branch = 9200 + index as i64;
        let app = fixture::app(70);
        let original = BranchIdentity::new(
            xid.clone(),
            branch,
            adapter.key().clone(),
            Some(app.clone()),
        )
        .unwrap();
        let identity = original.clone();
        let (sent, observed) = tokio::sync::oneshot::channel();
        let (resume, resumed) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = peer::bootstrap(&listener, original.resource().resource_id()).await;
            let id = peer::read_registration(
                &mut socket,
                original.resource().resource_id(),
                original.xid(),
                original.application_data(),
            )
            .await;
            sent.send(()).unwrap();
            match mode {
                "rejected" => {
                    peer::reject_registration(&mut socket, id).await;
                    peer::await_eof(&mut socket).await;
                }
                "lost" => {}
                _ => {
                    resumed.await.unwrap();
                    peer::reply_registered(&mut socket, id, branch).await;
                    for request in [41, 42] {
                        assert_eq!(
                            peer::callback(&mut socket, request, &original, BranchPhase::Rollback)
                                .await,
                            (ResultCode::Success, 8)
                        );
                    }
                }
            }
        });
        let invalid = [
            ("x".repeat(129), Some(app.clone())),
            (String::new(), Some(app.clone())),
            (" tcc:invalid".into(), Some(app.clone())),
            ("tcc:no-fence".into(), None),
            (
                "tcc:string-flag".into(),
                Some(r#"{"actionContext":{"useTCCFence":"true"}}"#.into()),
            ),
            ("tcc:json".into(), Some("{".into())),
        ];
        for (bad_xid, bad_app) in invalid {
            let error = adapter
                .register_and_try(&rm.client(), &bad_xid, bad_app)
                .await
                .unwrap_err();
            assert!(matches!(&error, MySqlTccTryError::Local { .. }));
            assert!(error.registered_branch().is_none());
        }
        if mode == "cancelled" {
            let resource = adapter.clone();
            let client = rm.client();
            let original_xid = xid.clone();
            let task = tokio::spawn(async move {
                resource
                    .register_and_try(&client, &original_xid, Some(app))
                    .await
            });
            observed.await.unwrap();
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            resume.send(()).unwrap();
            server.await.unwrap();
            assert_eq!(
                store
                    .lease(identity.clone(), BranchPhase::Rollback)
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Completed)
            );
            let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(status, 4);
            rm.shutdown().await.unwrap();
        } else {
            let error = adapter
                .register_and_try(&rm.client(), &xid, Some(app))
                .await
                .unwrap_err();
            assert!(error.registered_branch().is_none());
            match &error {
                MySqlTccTryError::Registration(cause) => {
                    if mode == "rejected" {
                        assert!(matches!(cause, RmCallError::DefiniteRejected { .. }));
                    } else {
                        assert!(cause.is_unknown_outcome());
                    }
                }
                other => panic!("registration classification changed: {other:?}"),
            }
            observed.await.unwrap();
            rm.shutdown().await.unwrap();
            server.await.unwrap();
            drop(resume);
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(count, 0);
        }
        assert!(fixture::business(&mut admin).await.is_empty());
        cases.push(serde_json::json!({"mode":mode,"xid":xid,"branch_id":null,"callback_branch_id":if mode=="cancelled"{Some(branch.to_string())}else{None},"invalid_inputs_rejected_before_registration":6,"original_register_requests":1,"successful_receipt_not_synthesized":true,"try_business_rows":0,"fence_rows":if mode=="cancelled"{1}else{0},"status":if mode=="cancelled"{Some(4)}else{None}}));
        source.close().await.unwrap();
    }
    fixture::record(
        "registration-boundaries",
        serde_json::json!({"cases":cases,"business":[]}),
    );
}

async fn wait_for_original_lock(admin: &mut sqlx::MySqlConnection) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='seata_rs_tcc_fence_v1'").fetch_one(&mut *admin).await.unwrap();
        if waits > 0 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original Cancel never waited for cancelled Try"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
#[ignore = "requires owned MySQL and actual cancellation before/after original COMMIT"]
async fn cancelled_try_before_commit_and_after_original_ok_preserves_distinct_database_outcomes() {
    let mut cases = Vec::new();
    for (index, stage) in ["before_commit", "after_commit"].into_iter().enumerate() {
        let (mut admin, seed_source, _) = fixture::seed().await;
        seed_source.close().await.unwrap();
        let proxy = if stage == "after_commit" {
            Some(
                XaCommandProxy::start(
                    std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
                    b"\x03COMMIT".to_vec(),
                )
                .await,
            )
        } else {
            None
        };
        let port = proxy.as_ref().map(|p| p.port);
        let source = MySqlDataSource::connect(
            fixture::owned_mysql::options(port)
                .operation_timeout(Duration::from_secs(6))
                .unwrap(),
        )
        .await
        .unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let action = if stage == "before_commit" {
            fixture::Action {
                entered: Some(entered.clone()),
                release: Some(release),
                ..Default::default()
            }
        } else {
            fixture::Action::default()
        };
        let adapter = Arc::new(
            MySqlTccResourceManager::new(
                source.clone(),
                "tcc-original-action".into(),
                Arc::new(action),
            )
            .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, store) = manager(adapter.clone(), &listener, &format!("cancel-{stage}")).await;
        let xid = format!("tcc:cancel:{stage}");
        let branch = 9300 + index as i64;
        let app = fixture::app(61 + index as i64);
        let identity = BranchIdentity::new(
            xid.clone(),
            branch,
            adapter.key().clone(),
            Some(app.clone()),
        )
        .unwrap();
        let original = identity.clone();
        let (start, ready) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = peer::bootstrap(&listener, original.resource().resource_id()).await;
            peer::registered(
                &mut socket,
                original.resource().resource_id(),
                original.xid(),
                original.application_data(),
                branch,
            )
            .await;
            ready.await.unwrap();
            for id in [1, 2] {
                assert_eq!(
                    peer::callback(&mut socket, id, &original, BranchPhase::Rollback).await,
                    (ResultCode::Success, 8)
                );
            }
        });
        let receipt = rm
            .client()
            .branch_register_with_receipt(BranchRegisterRequest {
                xid: Some(xid),
                branch_type: BranchType::Tcc,
                resource_id: Some(adapter.key().resource_id().into()),
                application_data: Some(app),
                lock_key: None,
            })
            .await
            .unwrap();
        assert_eq!(receipt.identity(), &identity);
        if let Some(proxy) = &proxy {
            proxy.arm();
        }
        let retained = receipt.clone();
        let trying = adapter.clone();
        let task = tokio::spawn(async move { trying.try_registered(retained).await });
        if stage == "before_commit" {
            tokio::time::timeout(Duration::from_secs(2), entered.notified())
                .await
                .unwrap();
            start.send(()).unwrap();
            wait_for_original_lock(&mut admin).await;
            assert!(fixture::business(&mut admin).await.is_empty());
        } else {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            let snapshot = loop {
                if let Some(value) = proxy.as_ref().unwrap().captured() {
                    break value;
                }
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "original Try COMMIT OK not actually captured"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            };
            assert_eq!(fixture::business(&mut admin).await, vec![(62, 7)]);
            let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(status, 1);
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            start.send(()).unwrap();
            server.await.unwrap();
            let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(status, 3);
            assert!(adapter.try_registered(receipt).await.is_err());
            assert!(fixture::business(&mut admin).await.is_empty());
            assert_eq!(
                store
                    .lease(identity.clone(), BranchPhase::Rollback)
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Completed)
            );
            cases.push(serde_json::json!({"stage":stage,"xid":identity.xid(),"branch_id":branch.to_string(),"status":3,"known_receipt_retained":true,"old_call_cancelled":true,"held_request":snapshot.0,"held_original_ok":snapshot.1,"committed_TRIED_and_business_observed":true,"late_try_rejected":true}));
            rm.shutdown().await.unwrap();
            source.close().await.unwrap();
            assert_eq!(proxy.unwrap().shutdown().await, 3);
            continue;
        };
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        server.await.unwrap();
        let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(status, 4);
        assert!(fixture::business(&mut admin).await.is_empty());
        assert!(adapter.try_registered(receipt).await.is_err());
        assert_eq!(
            store
                .lease(identity.clone(), BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        cases.push(serde_json::json!({"stage":stage,"xid":identity.xid(),"branch_id":branch.to_string(),"status":4,"known_receipt_retained":true,"old_call_cancelled":true,"actual_innodb_wait_observed":true,"late_try_rejected":true}));
        rm.shutdown().await.unwrap();
        source.close().await.unwrap();
    }
    fixture::record(
        "cancelled-try",
        serde_json::json!({"cases":cases,"business":[]}),
    );
}
