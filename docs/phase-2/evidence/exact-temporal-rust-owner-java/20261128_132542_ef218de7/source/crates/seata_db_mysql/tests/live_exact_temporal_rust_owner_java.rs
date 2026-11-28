//! Rust 原阶段一持锁、Java 精确 DATE/TIME 写入器竞争的真实联试。
#![cfg(feature = "experimental_changed_commit")]
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_datasource::{FieldValue, InternalTime, LocalDate};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_protocol::{v1::FrameLimits, GlobalStatus, PayloadLimits};
use seata_rm::{BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager};
use seata_tm::{TransactionConfig, TransactionManager};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::time::{sleep, Instant};

fn pool(endpoint: &str) -> ConnectionPoolConfig {
    ConnectionPoolConfig::new(
        endpoint.parse().unwrap(),
        ConnectionConfig::new(
            FrameLimits::new(
                65536,
                32767,
                128,
                PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
            )
            .unwrap(),
            64,
            Duration::from_secs(10),
            Duration::from_secs(10),
        )
        .unwrap(),
        1,
        64,
        ReconnectPolicy::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
fn save(out: &Path, name: &str, value: serde_json::Value) {
    std::fs::write(out.join(name), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}
async fn signal(out: &Path, name: &str) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while !out.join(name).exists() {
        assert!(
            Instant::now() < deadline,
            "missing controller signal {name}"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
#[ignore = "requires owned original Java TC/RM; run java_exact_temporal_rust_owner.py"]
async fn original_rust_owner_excludes_java_exact_temporal_writer() {
    tokio::time::timeout(Duration::from_secs(240), run())
        .await
        .unwrap();
}
async fn run() {
    let out = PathBuf::from(std::env::var("SEATA_REVERSE_TEMPORAL_OUT").unwrap());
    let code = std::env::var("SEATA_REVERSE_TEMPORAL_PROFILE")
        .unwrap()
        .parse()
        .unwrap();
    assert!(code == 5 || code == 6);
    let profile = MySqlJdbcProfile::from_storage_code(code).unwrap();
    let source = MySqlDataSource::connect(
        owned_mysql::options(None)
            .with_java_tc_mysql_db_lock_profile()
            .require_at_jdbc_profile_binding()
            .operation_timeout(Duration::from_secs(15))
            .unwrap(),
    )
    .await
    .unwrap();
    let manager = Arc::new(MySqlDataSourceManager::new(source.clone(), profile).unwrap());
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(manager.clone()).unwrap();
    let journal = FileBranchStore::open(
        out.join("branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let identity = RegistrationIdentity::new(
        "1.1.0".into(),
        "reverse-temporal-rust".into(),
        "cross_lock".into(),
        None,
    )
    .unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        identity.clone(),
        catalog,
        journal,
    )
    .request_timeout(Duration::from_secs(15))
    .callback_timeout(Duration::from_secs(20))
    .build()
    .unwrap();
    let tm =
        TransactionManager::builder(pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()), identity)
            .request_timeout(Duration::from_secs(20))
            .build()
            .unwrap();
    let cfg = TransactionConfig::builder("reverse-exact-temporal".into())
        .transaction_timeout(Duration::from_secs(120))
        .build()
        .unwrap();
    let mut global = tm.begin(&cfg).await.unwrap();
    let xid = global.xid().clone();
    let mut local = source.begin().await.unwrap();
    for (table, parameter) in [
        (
            "temporal_dates",
            FieldValue::LocalDate(LocalDate::parse_sql("1582-10-05").unwrap()),
        ),
        (
            "temporal_times",
            FieldValue::InternalTime(InternalTime::parse_sql("-25:01:02.123456").unwrap()),
        ),
    ] {
        let sql = SqlRecognizerFactory::recognize(&format!("UPDATE {table} SET v=v+10 WHERE k=?"))
            .unwrap();
        assert_eq!(local.execute(&sql, &[parameter]).await.unwrap(), 1);
    }
    let checked = manager
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    let receipt = checked
        .commit_changed_and_report(&manager, &rm.client())
        .await
        .unwrap();
    let branch = receipt.identity().branch_id();
    let mut admin = owned_mysql::admin().await;
    let witness: (u8, u8, i64) = sqlx::query_as("SELECT b.jdbc_profile,b.first_phase_committed,COUNT(*) FROM seata_rs_at_branch_v1 b JOIN undo_log u ON b.xid=CAST(u.xid AS BINARY) AND b.branch_id=u.branch_id WHERE b.xid=? AND b.branch_id=? AND b.undo_sha256=UNHEX(SHA2(u.rollback_info,256)) GROUP BY b.jdbc_profile,b.first_phase_committed")
        .bind(xid.as_str().as_bytes()).bind(branch).fetch_one(&mut admin).await.unwrap();
    assert_eq!(witness, (code, 1, 1));
    save(
        &out,
        "owner-held.json",
        serde_json::json!({"xid":xid.as_str(),"branch_id":branch,
        "profile":code,"resource":manager.key().resource_id(),"lock":receipt.lock_key(),"checksum_persisted":true}),
    );
    signal(&out, "owner-release").await;
    let commit = std::env::var("SEATA_REVERSE_TEMPORAL_DIRECTION").unwrap() == "commit";
    if commit {
        global.commit().await.unwrap();
    } else {
        global.rollback().await.unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
        assert!(Instant::now() < deadline);
        sleep(Duration::from_millis(20)).await;
    }
    let state: (u8, u8, u8, u8) = sqlx::query_as("SELECT phase,terminal,commit_terminal,first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?")
        .bind(xid.as_str().as_bytes()).bind(branch).fetch_one(&mut admin).await.unwrap();
    assert_eq!(state, if commit { (1, 0, 1, 1) } else { (2, 1, 0, 0) });
    save(
        &out,
        "owner-finished.json",
        serde_json::json!({"xid":xid.as_str(),"branch_id":branch,"state":state}),
    );
    signal(&out, "stop").await;
    tm.shutdown().await.unwrap();
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
