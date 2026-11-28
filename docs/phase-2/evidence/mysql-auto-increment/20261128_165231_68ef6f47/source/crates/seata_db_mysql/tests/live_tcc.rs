//! 正式 RM 与真实 MySQL 的 TCC 同事务 Fence 验收。
#![cfg(feature = "tcc")]
#![allow(clippy::unwrap_used)]
#[path = "support/tcc_managed_ack_cases.rs"]
mod ack_cases;
#[path = "support/xa_command_proxy.rs"]
mod command_proxy;
#[path = "support/tcc_configuration_transition_cases.rs"]
mod configuration_cases;
#[path = "support/tcc_dynamic_action.rs"]
mod dynamic_action;
#[path = "support/tcc_dynamic_cases.rs"]
mod dynamic_cases;
#[path = "support/tcc_dynamic_integrity_cases.rs"]
mod dynamic_integrity_cases;
#[path = "support/tcc_dynamic_setup_cases.rs"]
mod dynamic_setup_cases;
#[path = "support/tcc_managed_fixture.rs"]
mod fixture;
#[path = "support/tcc_managed_peer.rs"]
mod peer;
#[path = "support/tcc_managed_process_cases.rs"]
mod process_cases;
#[path = "support/tcc_dynamic_process_cases.rs"]
mod dynamic_process_cases;
#[path = "support/tcc_managed_registration_cases.rs"]
mod registration_cases;
use seata_db_mysql::MySqlTccResourceManager;
use seata_protocol::ResultCode;
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResolution, BranchResource,
    FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager,
};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

async fn manager(
    adapter: Arc<MySqlTccResourceManager>,
    listener: &TcpListener,
    name: &str,
) -> (ResourceManager, FileBranchStore) {
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter).unwrap();
    let store = FileBranchStore::open(
        fixture::folder(name).join("branches.log"),
        JournalOptions::new(32, 1048576, 65536, 32).unwrap(),
    )
    .await
    .unwrap();
    let manager = ResourceManager::builder(
        fixture::pool(listener.local_addr().unwrap()),
        fixture::registration(false),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(3))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    (manager, store)
}

#[tokio::test]
#[ignore = "requires owned MySQL and explicit TCC Fence schema"]
async fn original_tcc_try_and_terminal_effects_are_atomic_and_idempotent() {
    let (mut admin, source, adapter) = fixture::seed().await;
    let mut proofs = Vec::new();
    for (index, phase) in [BranchPhase::Commit, BranchPhase::Rollback]
        .into_iter()
        .enumerate()
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, store) = manager(adapter.clone(), &listener, &format!("normal-{index}")).await;
        let xid = format!("tcc:managed:{index}");
        let branch = 9007199254740995 + index as i64;
        let app = fixture::app(index as i64 + 1);
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
            let expected = if phase == BranchPhase::Commit { 5 } else { 8 };
            for id in [1, 2] {
                assert_eq!(
                    peer::callback(&mut socket, id, &original, phase).await,
                    (ResultCode::Success, expected)
                );
            }
        });
        let receipt = adapter
            .register_and_try(&rm.client(), &xid, Some(app))
            .await
            .unwrap();
        assert_eq!(receipt.identity(), &identity);
        assert_eq!(receipt.lock_key(), None);
        let state: u8 = sqlx::query_scalar(
            "SELECT status FROM seata_rs_tcc_fence_v1 WHERE xid=? AND branch_id=?",
        )
        .bind(xid.as_bytes())
        .bind(branch)
        .fetch_one(&mut admin)
        .await
        .unwrap();
        assert_eq!(state, 1);
        assert!(fixture::business(&mut admin)
            .await
            .contains(&(index as i32 + 1, 7)));
        let duplicate = adapter.try_registered(receipt.clone()).await.unwrap_err();
        assert_eq!(duplicate.registered_branch().unwrap().identity(), &identity);
        start.send(()).unwrap();
        server.await.unwrap();
        assert_eq!(
            store.lease(identity.clone(), phase).await.unwrap().state(),
            Some(BranchRecordState::Completed)
        );
        let state: u8 = sqlx::query_scalar(
            "SELECT status FROM seata_rs_tcc_fence_v1 WHERE xid=? AND branch_id=?",
        )
        .bind(xid.as_bytes())
        .bind(branch)
        .fetch_one(&mut admin)
        .await
        .unwrap();
        assert_eq!(state, if phase == BranchPhase::Commit { 2 } else { 3 });
        let opposite = if phase == BranchPhase::Commit {
            BranchPhase::Rollback
        } else {
            BranchPhase::Commit
        };
        assert_eq!(
            adapter.recover(&identity, opposite).await.unwrap(),
            BranchResolution::Unknown
        );
        proofs.push(serde_json::json!({"xid":xid,"branch_id":branch.to_string(),"status":state,"journal":"Completed","duplicate_try_rejected":true}));
        rm.shutdown().await.unwrap();
    }
    assert_eq!(fixture::business(&mut admin).await, vec![(1, 14)]);
    fixture::record(
        "normal",
        serde_json::json!({"branches":proofs,"business":[[1,14]]}),
    );
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned MySQL and explicit TCC Fence schema"]
async fn failed_actions_and_caught_transaction_errors_cannot_commit_fence_or_business() {
    let mut proof = Vec::new();
    for (index, action) in [
        fixture::Action {
            fail_try: true,
            ..Default::default()
        },
        fixture::Action {
            poison: true,
            ..Default::default()
        },
        fixture::Action {
            fail_confirm: true,
            ..Default::default()
        },
    ]
    .into_iter()
    .enumerate()
    {
        let (mut admin, source, _) = fixture::seed().await;
        let adapter = Arc::new(
            MySqlTccResourceManager::new(
                source.clone(),
                "tcc-original-action".into(),
                Arc::new(action),
            )
            .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, _) = manager(adapter.clone(), &listener, &format!("false-{index}")).await;
        let xid = format!("tcc:false:{index}");
        let branch = 6100 + index as i64;
        let app = fixture::app(21);
        let original = BranchIdentity::new(
            xid.clone(),
            branch,
            adapter.key().clone(),
            Some(app.clone()),
        )
        .unwrap();
        let identity = original.clone();
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
            let _ = stopped.await;
        });
        let result = adapter
            .register_and_try(&rm.client(), &xid, Some(app))
            .await;
        if index < 2 {
            let error = result.unwrap_err();
            assert!(matches!(
                &error,
                seata_db_mysql::MySqlTccTryError::Refused { .. }
            ));
            assert_eq!(error.registered_branch().unwrap().identity(), &identity);
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(count, 0);
            assert!(fixture::business(&mut admin).await.is_empty());
        } else {
            let receipt = result.unwrap();
            assert_eq!(
                adapter
                    .apply(receipt.identity(), BranchPhase::Commit)
                    .await
                    .unwrap(),
                BranchResolution::Unknown
            );
            let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(status, 1);
            assert_eq!(fixture::business(&mut admin).await, vec![(21, 7)]);
            let repaired = MySqlTccResourceManager::new(
                source.clone(),
                "tcc-original-action".into(),
                Arc::new(fixture::Action::default()),
            )
            .unwrap();
            assert_eq!(
                repaired
                    .recover(receipt.identity(), BranchPhase::Commit)
                    .await
                    .unwrap(),
                BranchResolution::Applied
            );
            assert_eq!(fixture::business(&mut admin).await, vec![(21, 14)]);
        }
        proof.push(serde_json::json!({"case":index,"xid":xid,"branch_id":branch.to_string(),"no_partial_effect":true}));
        rm.shutdown().await.unwrap();
        let _ = stop.send(());
        server.await.unwrap();
        source.close().await.unwrap();
    }
    fixture::record(
        "failed-actions",
        serde_json::json!({"cases":proof,"fresh_manager_confirmed":true,"business":[[21,14]]}),
    );
}

#[tokio::test]
#[ignore = "requires owned MySQL and explicit TCC Fence schema"]
async fn empty_cancel_from_original_callback_blocks_late_try_and_manual_identity_cannot_mint_fence()
{
    let (mut admin, source, adapter) = fixture::seed().await;
    let manual = BranchIdentity::new(
        "tcc:manual".into(),
        7000,
        adapter.key().clone(),
        Some(fixture::app(99)),
    )
    .unwrap();
    assert_eq!(
        adapter.apply(&manual, BranchPhase::Rollback).await.unwrap(),
        BranchResolution::Unknown
    );
    assert_eq!(
        adapter
            .recover(&manual, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, store) = manager(adapter.clone(), &listener, "early-cancel").await;
    let xid = "tcc:early";
    let branch = 7001;
    let app = fixture::app(99);
    let identity =
        BranchIdentity::new(xid.into(), branch, adapter.key().clone(), Some(app.clone())).unwrap();
    let original = identity.clone();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, original.resource().resource_id()).await;
        let request = peer::read_registration(
            &mut socket,
            original.resource().resource_id(),
            original.xid(),
            original.application_data(),
        )
        .await;
        for id in [41, 42] {
            assert_eq!(
                peer::callback(&mut socket, id, &original, BranchPhase::Rollback).await,
                (ResultCode::Success, 8)
            );
        }
        peer::reply_registered(&mut socket, request, branch).await;
    });
    let error = adapter
        .register_and_try(&rm.client(), xid, Some(app))
        .await
        .unwrap_err();
    assert_eq!(error.registered_branch().unwrap().identity(), &identity);
    server.await.unwrap();
    let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(status, 4);
    assert!(fixture::business(&mut admin).await.is_empty());
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    assert_eq!(
        adapter
            .recover(&identity, BranchPhase::Commit)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    fixture::record(
        "early-cancel",
        serde_json::json!({"xid":xid,"branch_id":branch.to_string(),"status":4,"late_try_rejected":true,"original_receipt_preserved":true,"manual_missing_is_unknown":true,"business":[]}),
    );
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned MySQL and physical TCC schema mutation"]
async fn changed_original_fence_identity_or_current_schema_never_executes_business() {
    let (mut admin, source, adapter) = fixture::seed().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(adapter.clone(), &listener, "corruption").await;
    let xid = "tcc:identity";
    let branch = 8001;
    let app = fixture::app(31);
    let original =
        BranchIdentity::new(xid.into(), branch, adapter.key().clone(), Some(app.clone())).unwrap();
    let identity = original.clone();
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
        let _ = stopped.await;
    });
    adapter
        .register_and_try(&rm.client(), xid, Some(app))
        .await
        .unwrap();
    let changes = [
        ("server_uuid", "X'00'"),
        ("schema_version", "2"),
        ("status", "255"),
        ("resource_id", "X'00'"),
        ("application_data", "X'7B7D'"),
    ];
    for (column, value) in changes {
        sqlx::raw_sql(&format!(
            "UPDATE seata_rs_tcc_fence_v1 SET {column}={value}"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        assert!(adapter.apply(&identity, BranchPhase::Commit).await.is_err());
        assert_eq!(fixture::business(&mut admin).await, vec![(31, 7)]);
        sqlx::query("UPDATE seata_rs_tcc_fence_v1 SET server_uuid=@@server_uuid,schema_version=1,status=1,resource_id=?,application_data=?").bind(identity.resource().resource_id().as_bytes()).bind(identity.application_data().unwrap().as_bytes()).execute(&mut admin).await.unwrap();
    }
    let schema_cases = [
        (
            "ALTER TABLE seata_rs_tcc_fence_v1 ADD INDEX unexpected(resource_id)",
            "ALTER TABLE seata_rs_tcc_fence_v1 DROP INDEX unexpected",
        ),
        (
            "ALTER TABLE seata_rs_tcc_fence_v1 ALTER COLUMN status SET DEFAULT 1",
            "ALTER TABLE seata_rs_tcc_fence_v1 ALTER COLUMN status DROP DEFAULT",
        ),
        (
            "ALTER TABLE tcc_managed ENGINE=MyISAM",
            "ALTER TABLE tcc_managed ENGINE=InnoDB",
        ),
        (
            "CREATE TRIGGER tcc_bad BEFORE UPDATE ON tcc_managed FOR EACH ROW SET NEW.v=999",
            "DROP TRIGGER tcc_bad",
        ),
    ];
    for (change, restore) in schema_cases {
        sqlx::raw_sql(change).execute(&mut admin).await.unwrap();
        assert_ne!(
            adapter.apply(&identity, BranchPhase::Commit).await.ok(),
            Some(BranchResolution::Applied)
        );
        assert_eq!(fixture::business(&mut admin).await, vec![(31, 7)]);
        sqlx::raw_sql(restore).execute(&mut admin).await.unwrap();
    }
    assert_eq!(
        adapter
            .recover(&identity, BranchPhase::Commit)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(fixture::business(&mut admin).await, vec![(31, 14)]);
    fixture::record(
        "identity",
        serde_json::json!({"xid":xid,"branch_id":branch.to_string(),"row_corruptions_rejected":5,"physical_changes_rejected":4,"restored_original_completed":true,"business":[[31,14]]}),
    );
    rm.shutdown().await.unwrap();
    let _ = stop.send(());
    server.await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned MySQL with observable InnoDB lock waits"]
async fn original_cancel_waits_for_actual_uncommitted_try_then_fences_late_business() {
    let (mut admin, source, _) = fixture::seed().await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let action = fixture::Action {
        entered: Some(entered.clone()),
        release: Some(release.clone()),
        ..Default::default()
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
    let (rm, store) = manager(adapter.clone(), &listener, "held-try").await;
    let xid = "tcc:held-try";
    let branch = 8101;
    let app = fixture::app(41);
    let identity =
        BranchIdentity::new(xid.into(), branch, adapter.key().clone(), Some(app.clone())).unwrap();
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
    let trying = adapter.clone();
    let client = rm.client();
    let task = tokio::spawn(async move { trying.register_and_try(&client, xid, Some(app)).await });
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    start.send(()).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='seata_rs_tcc_fence_v1'").fetch_one(&mut admin).await.unwrap();
        if waits > 0 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original Cancel never actually waited on Try Fence"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(fixture::business(&mut admin).await.is_empty());
    let visible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(visible, 0);
    release.notify_one();
    let receipt = task.await.unwrap().unwrap();
    assert_eq!(receipt.identity(), &identity);
    server.await.unwrap();
    let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(status, 3);
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
    fixture::record(
        "held-try",
        serde_json::json!({"xid":xid,"branch_id":branch.to_string(),"actual_innodb_wait_observed":true,"uncommitted_fence_and_business_invisible":true,"status":3,"late_try_rejected":true,"business":[]}),
    );
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
