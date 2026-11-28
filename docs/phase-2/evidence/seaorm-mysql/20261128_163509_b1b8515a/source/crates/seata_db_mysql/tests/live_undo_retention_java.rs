//! 真实原 TC 的单向清理与同一 RM 通道后续分支回调。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
use seata_protocol::{v1::FrameLimits, GlobalStatus, PayloadLimits};
use seata_rm::{BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager};
use seata_tm::{TransactionConfig, TransactionManager};
use seata_transport::{
    ConnectionConfig, ConnectionIdleConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
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
        .unwrap()
        .with_idle(ConnectionIdleConfig::new(Duration::from_secs(5), Duration::from_secs(30)).unwrap()),
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
    let end = Instant::now() + Duration::from_secs(120);
    while !out.join(name).exists() {
        assert!(Instant::now() < end, "missing {name}");
        sleep(Duration::from_millis(20)).await;
    }
}
#[tokio::test]
#[ignore = "requires owned original TC/MySQL; run undo_retention_java.py"]
async fn original_tc_cleanup_preserves_channel_and_matches_retention_rows() {
    tokio::time::timeout(Duration::from_secs(420), run())
        .await
        .unwrap();
}
async fn run() {
    let out = PathBuf::from(std::env::var("SEATA_UNDO_RETENTION_OUT").unwrap());
    let source = MySqlDataSource::connect(
        owned_mysql::options(None)
            .with_java_tc_mysql_db_lock_profile()
            .require_at_jdbc_profile_binding()
            .operation_timeout(Duration::from_secs(15))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let journal = FileBranchStore::open(
        out.join("branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let id = RegistrationIdentity::new(
        "1.1.0".into(),
        "undo-retention-rust".into(),
        "cross_lock".into(),
        None,
    )
    .unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        id.clone(),
        catalog,
        journal,
    )
    .request_timeout(Duration::from_secs(15))
    .callback_timeout(Duration::from_secs(20))
    .build()
    .unwrap();
    let tm = TransactionManager::builder(pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()), id)
        .request_timeout(Duration::from_secs(15))
        .build()
        .unwrap();
    let mut admin = owned_mysql::admin().await;
    save(
        &out,
        "ready.json",
        serde_json::json!({"resource":adapter.key().resource_id()}),
    );
    let end = Instant::now() + Duration::from_secs(260);
    loop {
        let rows: Vec<String> = sqlx::query_scalar("SELECT xid FROM undo_log ORDER BY xid")
            .fetch_all(&mut admin)
            .await
            .unwrap();
        if rows == ["keep-micro", "keep-noon", "keep-normal", "keep-young-fence"] {
            save(&out, "cleaned.json", serde_json::json!({"remaining":rows}));
            break;
        }
        assert!(Instant::now() < end, "original TC cleanup did not execute");
        sleep(Duration::from_millis(50)).await;
    }
    signal(&out, "probe").await;
    let cfg = TransactionConfig::builder("after-undo-retention".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut global = tm.begin(&cfg).await.unwrap();
    let xid = global.xid().clone();
    let mut local = source.begin().await.unwrap();
    assert_eq!(
        local
            .execute(
                &SqlRecognizerFactory::recognize("UPDATE cleanup_probe SET v=v+1 WHERE id=1")
                    .unwrap(),
                &[]
            )
            .await
            .unwrap(),
        1
    );
    let prepared = adapter
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .unwrap();
    let branch = prepared.registered_branch().unwrap().identity().branch_id();
    prepared.rollback().await.unwrap();
    global.rollback().await.unwrap();
    let end = Instant::now() + Duration::from_secs(15);
    while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
        assert!(Instant::now() < end);
        sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT v FROM cleanup_probe WHERE id=1")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        10
    );
    save(
        &out,
        "probe.json",
        serde_json::json!({"xid":xid.as_str(),"branch_id":branch,"resource":adapter.key().resource_id(),"value":10}),
    );
    signal(&out, "stop").await;
    tm.shutdown().await.unwrap();
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
