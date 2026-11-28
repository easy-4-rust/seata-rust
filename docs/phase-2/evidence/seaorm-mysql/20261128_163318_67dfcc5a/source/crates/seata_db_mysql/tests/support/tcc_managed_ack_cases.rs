//! 截留真实 COMMIT OK 后，使用原身份与旧日志在新 RM 中恢复；不伪造数据库成功。
#![allow(clippy::unwrap_used)]
use super::{command_proxy::XaCommandProxy, fixture, manager, peer};
use seata_db_mysql::{MySqlDataSource, MySqlTccResourceManager, MySqlTccTryError};
use seata_protocol::ResultCode;
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResource};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned MySQL and real original COMMIT OK loss"]
async fn lost_try_confirm_and_cancel_commit_ok_recover_original_fence_without_business_replay() {
    let mut proofs = Vec::new();
    for (index, stage) in ["try", "confirm", "cancel"].into_iter().enumerate() {
        let (mut admin, seed_source, _) = fixture::seed().await;
        seed_source.close().await.unwrap();
        let upstream = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
        let proxy = XaCommandProxy::start(upstream, b"\x03COMMIT".to_vec()).await;
        let source = MySqlDataSource::connect(
            fixture::owned_mysql::options(Some(proxy.port))
                .operation_timeout(Duration::from_millis(750))
                .unwrap(),
        )
        .await
        .unwrap();
        let adapter = Arc::new(
            MySqlTccResourceManager::new(
                source.clone(),
                "tcc-original-action".into(),
                Arc::new(fixture::Action::default()),
            )
            .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let name = format!("ack-{stage}");
        let (rm, store) = manager(adapter.clone(), &listener, &name).await;
        let xid = format!("tcc:ack:{stage}");
        let branch = 9001 + index as i64;
        let app = fixture::app(51 + index as i64);
        let identity = BranchIdentity::new(
            xid.clone(),
            branch,
            adapter.key().clone(),
            Some(app.clone()),
        )
        .unwrap();
        let original = identity.clone();
        let phase = if stage == "cancel" {
            BranchPhase::Rollback
        } else {
            BranchPhase::Commit
        };
        let (start, ready) = tokio::sync::oneshot::channel();
        let (first, observed) = tokio::sync::oneshot::channel();
        let (stop, stopped) = tokio::sync::oneshot::channel();
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
            if stage != "try" {
                assert_eq!(
                    peer::callback(&mut socket, 1, &original, phase).await,
                    (
                        ResultCode::Success,
                        if phase == BranchPhase::Commit { 6 } else { 9 }
                    )
                );
            }
            first.send(()).unwrap();
            let _ = stopped.await;
        });
        if stage == "try" {
            proxy.arm();
        }
        let result = adapter
            .register_and_try(&rm.client(), &xid, Some(app))
            .await;
        let receipt = if stage == "try" {
            let error = result.unwrap_err();
            assert!(matches!(
                &error,
                MySqlTccTryError::UnknownCommit { .. }
                    | MySqlTccTryError::Deadline {
                        try_commit_may_have_effect: true,
                        ..
                    }
            ));
            error.registered_branch().unwrap().clone()
        } else {
            result.unwrap()
        };
        assert_eq!(receipt.identity(), &identity);
        if stage != "try" {
            proxy.arm_after(if stage == "cancel" { 1 } else { 0 });
        }
        start.send(()).unwrap();
        observed.await.unwrap();
        let Some((request, ok)) = proxy.captured() else {
            panic!("actual original COMMIT OK not captured");
        };
        assert_eq!(&request[4..], b"\x03COMMIT");
        assert_eq!(ok[4], 0);
        let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(
            status,
            if stage == "try" {
                1
            } else if stage == "confirm" {
                2
            } else {
                3
            }
        );
        if stage != "try" {
            assert_eq!(
                store.lease(identity.clone(), phase).await.unwrap().state(),
                Some(BranchRecordState::Pending)
            );
        }
        assert_eq!(
            fixture::business(&mut admin).await,
            if stage == "cancel" {
                vec![]
            } else {
                vec![(51 + index as i32, if stage == "try" { 7 } else { 14 })]
            }
        );
        rm.shutdown().await.unwrap();
        let _ = stop.send(());
        server.await.unwrap();
        drop(store);
        drop(adapter);
        source.close().await.unwrap();
        let commits = proxy.shutdown().await;
        assert_eq!(
            commits,
            if stage == "try" {
                1
            } else if stage == "cancel" {
                3
            } else {
                2
            }
        );
        let fresh_source = MySqlDataSource::connect(fixture::owned_mysql::options(None))
            .await
            .unwrap();
        let fresh = Arc::new(
            MySqlTccResourceManager::new(
                fresh_source.clone(),
                "tcc-original-action".into(),
                Arc::new(fixture::Action::default()),
            )
            .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (fresh_rm, reopened) = manager(fresh, &listener, &name).await;
        let original = identity.clone();
        if stage != "try" {
            assert_eq!(
                reopened
                    .lease(identity.clone(), phase)
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Pending)
            );
        }
        let server = tokio::spawn(async move {
            let mut socket = peer::bootstrap(&listener, original.resource().resource_id()).await;
            for id in [2, 3] {
                assert_eq!(
                    peer::callback(&mut socket, id, &original, phase).await,
                    (
                        ResultCode::Success,
                        if phase == BranchPhase::Commit { 5 } else { 8 }
                    )
                );
            }
        });
        tokio::time::timeout(Duration::from_secs(10), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            reopened
                .lease(identity.clone(), phase)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        assert_eq!(
            fixture::business(&mut admin).await,
            if stage == "cancel" {
                vec![]
            } else {
                vec![(51 + index as i32, 14)]
            }
        );
        proofs.push(serde_json::json!({"stage":stage,"xid":xid,"branch_id":branch.to_string(),"held_request":request,"held_original_ok":ok,"database_status_before_recovery":status,"old_journal":if stage=="try"{"NoCallbackYet"}else{"Pending"},"journal":"Completed","proxy_commit_commands":commits,"fresh_manager_and_source":true}));
        fresh_rm.shutdown().await.unwrap();
        fresh_source.close().await.unwrap();
    }
    fixture::record(
        "ack-loss",
        serde_json::json!({"cases":proofs,"business":[]}),
    );
}
