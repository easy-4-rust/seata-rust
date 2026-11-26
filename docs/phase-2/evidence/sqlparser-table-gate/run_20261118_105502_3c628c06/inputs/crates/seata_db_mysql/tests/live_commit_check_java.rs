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
#[ignore = "requires pinned original Java TC, raw passthrough observer and owned MySQL"]
async fn original_java_tc_preparation_keeps_normal_undo_local_then_rolls_back_before_the_real_callback(
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
    let cfg = TransactionConfig::builder("mysql-callback-unknown-registration".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut transaction = tm.begin(&cfg).await.unwrap();
    let xid = transaction.xid().clone();
    let mut local = ds.begin().await.unwrap();
    local
        .execute(
            &SqlRecognizerFactory::recognize(
                "INSERT INTO undo_work(id,v) VALUES(1,'uncommitted_java_tc')",
            )
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
    let prepared = adapter_for_prepare
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .expect("actual Java TC preparation must produce a real receipt");
    let prepared = prepared.lock_commit_preconditions().await.unwrap();
    let original_branch = prepared.registered_branch().unwrap().identity().branch_id();
    assert!(original_branch > 0);
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
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT log_status FROM undo_log WHERE xid=? AND branch_id=?")
            .bind(xid.as_str())
            .bind(original_branch)
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        0
    );
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut admin)
        .await
        .unwrap();
    prepared.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    let end = transaction.rollback().await.unwrap();
    match end {
        TransactionCompletion::Observed { outcome, .. } => {
            assert_eq!(outcome.status(), GlobalStatus::Rollbacked)
        }
        other => panic!("{other:?}"),
    };
    // 原成功回执和真实回调的分支号必须一致，不从代理借用分配结果。
    let row:(Vec<u8>,i64,u16,u8,u8,u8,Option<Vec<u8>>)=sqlx::query_as("SELECT xid,branch_id,schema_version,registration_origin,phase,terminal,lock_key FROM seata_rs_at_branch_v1").fetch_one(&mut admin).await.unwrap();
    assert_eq!(row.0, xid.as_str().as_bytes());
    assert_eq!(row.1, original_branch);
    assert_eq!(
        (row.2, row.3, row.4, row.5, row.6),
        (2, 1, 2, 1, Some(b"undo_work:1".to_vec()))
    );
    let identity = BranchIdentity::new(xid.as_str().into(), row.1, key.clone(), None).unwrap();
    assert_eq!(
        store
            .lease(identity, BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT log_status FROM undo_log WHERE xid=? AND branch_id=?")
            .bind(xid.as_str())
            .bind(row.1)
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        1
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    fixture::save(
        "java-first-phase-preparation.json",
        &serde_json::json!({"xid":xid.as_str(),"branch_id":row.1,"resource_id":key.resource_id(),"global_result":"Rollbacked","file_result":"Completed","schema_version":2,"origin":1,"phase":2,"terminal":1,"lock_key":"undo_work:1","undo_log_status":1,"business_rows":0,"registration_result":"Success","local_rollback_before_TC_end":true,"uncommitted_Normal_observed":true,"public_business_commit":false,"commit_preconditions_locked":true}),
    );
    // Finished 仅用于确认 TC 已清理当前会话，不把它单独视为历史回滚证明。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let status = tm.get_status(&xid).await.unwrap().status();
        if status == GlobalStatus::Finished {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
}
