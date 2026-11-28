//! 原报告清单改写/结构损坏不绕过旧 Completed；恢复原字节后同一分支继续。
#![allow(clippy::unwrap_used)]
use super::dynamic_action as action;
use super::{fixture, manager, peer};
use seata_db_mysql::MySqlTccResourceManager;
use seata_protocol::ResultCode;
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResolution, BranchResource};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned MySQL and real persisted report identity/shape corruption"]
async fn modified_report_versions_never_reuse_completed_and_original_bytes_restore_same_branch() {
    let (mut admin, source, _) = fixture::seed().await;
    sqlx::raw_sql("DROP TABLE IF EXISTS seata_rs_tcc_context_v1")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/020_seata_rs_tcc_context_v1.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let adapter = Arc::new(
        MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            Arc::new(action::TccDynamicAction {
                immediate: true,
                precision_only: false,
                catch_report_error: false,
                failure_mode: None,
            }),
        )
        .unwrap()
        .with_context_reporting(),
    );
    let original = BranchIdentity::new(
        "tcc:dynamic:integrity".into(),
        9701,
        adapter.key().clone(),
        Some(fixture::app(1)),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, store) = manager(adapter.clone(), &listener, "dynamic-integrity").await;
    let id = original.clone();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, id.resource().resource_id()).await;
        peer::registered(
            &mut socket,
            id.resource().resource_id(),
            id.xid(),
            id.application_data(),
            9701,
        )
        .await;
        let mut reports = Vec::new();
        for _ in 0..2 {
            let (request, app) = peer::read_context_report(&mut socket, &id).await;
            peer::reply_context_report(&mut socket, request, ResultCode::Success).await;
            reports.push(app);
        }
        reports
    });
    let receipt = adapter
        .register_and_try(
            &rm.client(),
            original.xid(),
            original.application_data().map(str::to_owned),
        )
        .await
        .unwrap();
    assert_eq!(receipt.identity(), &original);
    let reports = server.await.unwrap();
    let fence_protocol: u16 =
        sqlx::query_scalar("SELECT schema_version FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!(fence_protocol, 2);
    let plain = MySqlTccResourceManager::new(
        source.clone(),
        "tcc-original-action".into(),
        Arc::new(fixture::Action::default()),
    )
    .unwrap();
    assert!(
        matches!(
            plain.recover(&original, BranchPhase::Commit).await,
            Err(seata_rm::RmError::IdentityConflict)
        ),
        "static mode must reject report-enabled records before using old parameters"
    );
    let current = BranchIdentity::new(
        original.xid().into(),
        9701,
        original.resource().clone(),
        Some(reports[1].clone()),
    )
    .unwrap();
    let stale = BranchIdentity::new(
        original.xid().into(),
        9701,
        original.resource().clone(),
        Some(reports[0].clone()),
    )
    .unwrap();
    assert_eq!(
        adapter.recover(&stale, BranchPhase::Commit).await.unwrap(),
        BranchResolution::Unknown
    );
    assert_eq!(
        adapter
            .recover(&original, BranchPhase::Commit)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    type Snapshot = (u16, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, u16, Vec<u8>);
    let rows:Vec<Snapshot>=sqlx::query_as("SELECT revision,resource_id,registration_data,application_data,server_uuid,schema_version,checksum FROM seata_rs_tcc_context_v1 ORDER BY revision").fetch_all(&mut admin).await.unwrap();
    let mutations=[
        ("payload","UPDATE seata_rs_tcc_context_v1 SET application_data=CONCAT(application_data,0x20) WHERE revision=2"),
        ("registration","UPDATE seata_rs_tcc_context_v1 SET registration_data=CONCAT(registration_data,0x20) WHERE revision=2"),
        ("resource","UPDATE seata_rs_tcc_context_v1 SET resource_id=0x6F74686572 WHERE revision=2"),
        ("instance","UPDATE seata_rs_tcc_context_v1 SET server_uuid=REPEAT('0',36) WHERE revision=2"),
        ("version","UPDATE seata_rs_tcc_context_v1 SET schema_version=2 WHERE revision=2"),
        ("checksum","UPDATE seata_rs_tcc_context_v1 SET checksum=REPEAT('0',32) WHERE revision=2"),
        ("high_revision","UPDATE seata_rs_tcc_context_v1 SET revision=65535 WHERE revision=2"),
        ("gap","DELETE FROM seata_rs_tcc_context_v1 WHERE revision=1"),
        ("missing_manifest","DELETE FROM seata_rs_tcc_context_v1"),
    ];
    let mut rejected = Vec::new();
    for (name, sql) in mutations {
        sqlx::raw_sql(sql).execute(&mut admin).await.unwrap();
        assert!(!matches!(
            adapter.recover(&current, BranchPhase::Commit).await,
            Ok(BranchResolution::Applied)
        ));
        assert_eq!(fixture::business(&mut admin).await, vec![(1, 7)]);
        rejected.push(name);
        sqlx::raw_sql("DELETE FROM seata_rs_tcc_context_v1")
            .execute(&mut admin)
            .await
            .unwrap();
        for row in &rows {
            sqlx::query("INSERT INTO seata_rs_tcc_context_v1(xid,branch_id,revision,resource_id,registration_data,application_data,server_uuid,schema_version,checksum) VALUES(?,?,?,?,?,?,?,?,?)").bind(original.xid().as_bytes()).bind(9701i64).bind(row.0).bind(&row.1).bind(&row.2).bind(&row.3).bind(&row.4).bind(row.5).bind(&row.6).execute(&mut admin).await.unwrap();
        }
    }
    for (name,alter,restore) in [
        ("extra_index","ALTER TABLE seata_rs_tcc_context_v1 ADD INDEX unexpected(revision)","ALTER TABLE seata_rs_tcc_context_v1 DROP INDEX unexpected"),
        ("changed_default","ALTER TABLE seata_rs_tcc_context_v1 ALTER COLUMN schema_version SET DEFAULT 2","ALTER TABLE seata_rs_tcc_context_v1 ALTER COLUMN schema_version SET DEFAULT 1"),
        ("nullable","ALTER TABLE seata_rs_tcc_context_v1 MODIFY application_data MEDIUMBLOB NULL","ALTER TABLE seata_rs_tcc_context_v1 MODIFY application_data MEDIUMBLOB NOT NULL"),
        ("engine","ALTER TABLE seata_rs_tcc_context_v1 ENGINE=MyISAM","ALTER TABLE seata_rs_tcc_context_v1 ENGINE=InnoDB"),
        ("trigger","CREATE TRIGGER dynamic_context_trigger BEFORE UPDATE ON seata_rs_tcc_context_v1 FOR EACH ROW SET NEW.revision=NEW.revision","DROP TRIGGER dynamic_context_trigger"),
    ] {
        sqlx::raw_sql(alter).execute(&mut admin).await.unwrap();
        assert!(!matches!(adapter.recover(&current,BranchPhase::Commit).await,Ok(BranchResolution::Applied)));
        assert_eq!(fixture::business(&mut admin).await,vec![(1,7)]);rejected.push(name);
        sqlx::raw_sql(restore).execute(&mut admin).await.unwrap();
    }
    assert_eq!(rejected.len(), 14);
    rm.shutdown().await.unwrap();
    drop(store);
    let fresh = Arc::new(
        MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            Arc::new(fixture::Action::default()),
        )
        .unwrap()
        .with_context_reporting(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, store) = manager(fresh, &listener, "dynamic-integrity").await;
    let (first, completed) = tokio::sync::oneshot::channel();
    let (bad, corrupted) = tokio::sync::oneshot::channel();
    let (failed, rejected_callback) = tokio::sync::oneshot::channel();
    let (restore, restored) = tokio::sync::oneshot::channel();
    let latest_identity = current.clone();
    let (hash_restored, hash_ready) = tokio::sync::oneshot::channel();
    let (status_bad, status_corrupted) = tokio::sync::oneshot::channel();
    let (status_failed, status_rejected) = tokio::sync::oneshot::channel();
    let (status_restore, status_restored) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, current.resource().resource_id()).await;
        assert_eq!(
            peer::callback(&mut socket, -601, &current, BranchPhase::Commit).await,
            (ResultCode::Success, 5)
        );
        first.send(()).unwrap();
        corrupted.await.unwrap();
        assert_eq!(
            peer::callback(&mut socket, -602, &current, BranchPhase::Commit).await,
            (ResultCode::Failed, 6)
        );
        failed.send(()).unwrap();
        restored.await.unwrap();
        assert_eq!(
            peer::callback(&mut socket, -603, &current, BranchPhase::Commit).await,
            (ResultCode::Success, 5)
        );
        hash_restored.send(()).unwrap();
        status_corrupted.await.unwrap();
        assert_eq!(
            peer::callback(&mut socket, -604, &current, BranchPhase::Commit).await,
            (ResultCode::Failed, 6)
        );
        status_failed.send(()).unwrap();
        status_restored.await.unwrap();
        assert_eq!(
            peer::callback(&mut socket, -605, &current, BranchPhase::Commit).await,
            (ResultCode::Success, 5)
        );
    });
    completed.await.unwrap();
    assert_eq!(
        store
            .lease(original.clone(), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    sqlx::query("UPDATE seata_rs_tcc_context_v1 SET checksum=REPEAT('0',32) WHERE revision=2")
        .execute(&mut admin)
        .await
        .unwrap();
    bad.send(()).unwrap();
    rejected_callback.await.unwrap();
    assert_eq!(fixture::business(&mut admin).await, vec![(1, 18)]);
    sqlx::query("UPDATE seata_rs_tcc_context_v1 SET checksum=? WHERE revision=2")
        .bind(&rows[2].6)
        .execute(&mut admin)
        .await
        .unwrap();
    restore.send(()).unwrap();
    hash_ready.await.unwrap();
    sqlx::query("UPDATE seata_rs_tcc_fence_v1 SET status=1")
        .execute(&mut admin)
        .await
        .unwrap();
    status_bad.send(()).unwrap();
    status_rejected.await.unwrap();
    assert_eq!(fixture::business(&mut admin).await, vec![(1, 18)]);
    sqlx::query("UPDATE seata_rs_tcc_fence_v1 SET status=2")
        .execute(&mut admin)
        .await
        .unwrap();
    status_restore.send(()).unwrap();
    server.await.unwrap();
    assert_eq!(fixture::business(&mut admin).await, vec![(1, 18)]);
    rm.shutdown().await.unwrap();
    drop(store);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plain = Arc::new(
        MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            Arc::new(fixture::Action::default()),
        )
        .unwrap(),
    );
    let (static_rm, static_store) = manager(plain, &listener, "dynamic-integrity").await;
    let old = original.clone();
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, old.resource().resource_id()).await;
        assert_eq!(
            peer::callback(&mut socket, -606, &old, BranchPhase::Commit).await,
            (ResultCode::Failed, 7)
        );
    });
    server.await.unwrap();
    static_rm.shutdown().await.unwrap();
    drop(static_store);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (restored_rm, _) = manager(adapter, &listener, "dynamic-integrity").await;
    let server = tokio::spawn(async move {
        let mut socket = peer::bootstrap(&listener, latest_identity.resource().resource_id()).await;
        assert_eq!(
            peer::callback(&mut socket, -607, &latest_identity, BranchPhase::Commit).await,
            (ResultCode::Success, 5)
        );
    });
    server.await.unwrap();
    assert_eq!(fixture::business(&mut admin).await, vec![(1, 18)]);
    restored_rm.shutdown().await.unwrap();
    source.close().await.unwrap();
    fixture::record(
        "dynamic-report-integrity",
        serde_json::json!({"xid":original.xid(),"branch_id":9701,"rejected":rejected,"old_known_context_retryable":true,"static_downgrade_rejected":true,"static_completed_downgrade_rejected":true,"inconsistent_terminal_fence_not_reused":true,"fence_protocol":fence_protocol,"completed_reuse_rejected_bad_context":true,"original_bytes_restored_same_branch_completed":true,"journal":"Completed","business":[[1,18]],"reports":reports,"versions":3,"whole_plan_complete":false}),
    );
}
