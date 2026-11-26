//! 固定原 Java TC + 正式 TM/RM + 实际 MySQL；准备后回滚原本地事务，再观察原 TC 回调。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_protocol::GlobalStatus;
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResource, FileBranchStore,
    JournalOptions, ResourceCatalog, ResourceManager,
};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
};
use sqlx::Row;
use std::{path::PathBuf, sync::Arc, time::Duration};

fn pool(endpoint: &str) -> ConnectionPoolConfig {
    let connection = ConnectionConfig::new(
        seata_protocol::v1::FrameLimits::new(
            65536,
            32767,
            128,
            seata_protocol::PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
        )
        .unwrap(),
        16,
        Duration::from_secs(3),
        Duration::from_secs(3),
    )
    .unwrap();
    ConnectionPoolConfig::new(
        endpoint.parse().unwrap(),
        connection,
        1,
        16,
        ReconnectPolicy::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
fn registration() -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some("phase0-local-only".into()),
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "requires pinned original Java TC and owned MySQL"]
async fn original_java_tc_commit_authorizes_and_cleans_manually_committed_fixture_without_business_replay(
) {
    let mut admin = fixture::reset().await;
    sqlx::raw_sql(include_str!("../migrations/001_seata_rs_at_branch_v1.sql"))
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/003_seata_rs_at_commit_decision.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(ds.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let key = adapter.key().clone();
    let out = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let store = FileBranchStore::open(
        out.join("java-branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    let adapter_for_prepare = adapter.clone();
    catalog.register(adapter).unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_millis(800))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let tm = TransactionManager::builder(
        pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
        registration(),
    )
    .request_timeout(Duration::from_secs(10))
    .build()
    .unwrap();
    let cfg = TransactionConfig::builder("mysql-commit-direction-fixture".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();

    let mut transaction = tm.begin(&cfg).await.unwrap();
    let xid = transaction.xid().clone();
    let mut local = ds.begin().await.unwrap();
    local
        .execute(
            &SqlRecognizerFactory::recognize(
                "INSERT INTO undo_work(id,v) VALUES(1,'committed_java_fixture')",
            )
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
    let prepared = adapter_for_prepare
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    let original_branch = prepared.registered_branch().unwrap().identity().branch_id();
    assert_eq!(
        prepared.registered_branch().unwrap().lock_key(),
        Some("undo_work:1")
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL READ UNCOMMITTED")
        .execute(&mut admin)
        .await
        .unwrap();
    let original_normal: (String, Vec<u8>, i32) = sqlx::query_as(
        "SELECT context,rollback_info,log_status FROM undo_log WHERE xid=? AND branch_id=?",
    )
    .bind(xid.as_str())
    .bind(original_branch)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(original_normal.2, 0);
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut admin)
        .await
        .unwrap();
    prepared.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    // 已提交场景只由自有管理连接显式重建；并非尚未开放的正式一阶段业务COMMIT。
    sqlx::query("INSERT INTO undo_work VALUES(1,'committed_java_fixture')")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("INSERT INTO undo_log VALUES(?,?,?,?,0,now(6),now(6))")
        .bind(original_branch)
        .bind(xid.as_str())
        .bind(&original_normal.0)
        .bind(&original_normal.1)
        .execute(&mut admin)
        .await
        .unwrap();
    let sentinel = if original_branch == 1 { 2_i64 } else { 1_i64 };
    sqlx::query("INSERT INTO undo_log VALUES(?,?,'serializer=sentinel',?,0,now(6),now(6))")
        .bind(sentinel)
        .bind(xid.as_str())
        .bind(b"sentinel".as_slice())
        .execute(&mut admin)
        .await
        .unwrap();
    let before:(u16,u8,u8,u8,u8)=sqlx::query_as("SELECT schema_version,phase,terminal,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1")
        .fetch_one(&mut admin).await.unwrap();
    assert_eq!(before, (3, 0, 0, 0, 0));
    match transaction.commit().await.unwrap() {
        TransactionCompletion::Observed { outcome, .. } => {
            assert_eq!(outcome.status(), GlobalStatus::Committed)
        }
        other => panic!("{other:?}"),
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let markers: (u8, u8) =
            sqlx::query_as("SELECT commit_authorized,commit_terminal FROM seata_rs_at_branch_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
        if markers == (1, 1) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "actual original Java commit callback required"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let identity =
        BranchIdentity::new(xid.as_str().into(), original_branch, key.clone(), None).unwrap();
    assert_eq!(
        store
            .lease(identity, BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    let row=sqlx::query("SELECT xid,branch_id,schema_version,registration_origin,phase,terminal,commit_authorized,commit_terminal,lock_key FROM seata_rs_at_branch_v1").fetch_one(&mut admin).await.unwrap();
    assert_eq!(row.get::<Vec<u8>, _>("xid"), xid.as_str().as_bytes());
    assert_eq!(row.get::<i64, _>("branch_id"), original_branch);
    assert_eq!(
        (
            row.get::<u16, _>("schema_version"),
            row.get::<u8, _>("registration_origin"),
            row.get::<u8, _>("phase"),
            row.get::<u8, _>("terminal"),
            row.get::<u8, _>("commit_authorized"),
            row.get::<u8, _>("commit_terminal"),
            row.get::<Option<Vec<u8>>, _>("lock_key")
        ),
        (3, 1, 1, 0, 1, 1, Some(b"undo_work:1".to_vec()))
    );
    let remaining: Vec<i64> = sqlx::query_scalar("SELECT branch_id FROM undo_log")
        .fetch_all(&mut admin)
        .await
        .unwrap();
    assert_eq!(remaining, [sentinel]);
    let business: String = sqlx::query_scalar("SELECT v FROM undo_work WHERE id=1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(business, "committed_java_fixture");
    loop {
        let status = tm.get_status(&xid).await.unwrap().status();
        if status == GlobalStatus::Finished {
            break;
        }
        assert!(matches!(
            status,
            GlobalStatus::AsyncCommitting | GlobalStatus::Committed
        ));
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    fixture::save(
        "java-commit-direction.json",
        &serde_json::json!({"xid":xid.as_str(),"branch_id":original_branch,"resource_id":key.resource_id(),"global_result":"Committed","file_result":"Completed","schema_version":3,"origin":1,"phase":1,"rollback_terminal":0,"commit_authorized":1,"commit_terminal":1,"lock_key":"undo_work:1","business_rows":1,"business":business,"remaining_undo":[sentinel],"registration_result":"Success","local_rollback_before_fixture_commit":true,"fixture_committed_via_admin":true,"commit_preconditions_locked":true,"uncommitted_Normal_observed":true,"business_replayed":false,"public_business_commit":false}),
    );
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
}
