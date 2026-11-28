//! 原 MySQL 冷缓存 RSA 完整认证及正式 AT 原子 Undo/回调烟测。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(feature = "experimental_changed_commit")]
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager, MySqlOptions};
use seata_protocol::{v1::FrameLimits, GlobalStatus, PayloadLimits};
use seata_rm::{BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager};
use seata_tm::{TransactionConfig, TransactionManager};
use seata_transport::{
    ConnectionConfig, ConnectionIdleConfig, ConnectionPoolConfig, ReconnectPolicy,
    RegistrationIdentity,
};
use sha2::{Digest, Sha256};
use sqlx::Connection;
use std::{path::PathBuf, sync::Arc, time::Duration};
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
        .with_idle(
            ConnectionIdleConfig::new(Duration::from_secs(5), Duration::from_secs(30)).unwrap(),
        ),
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

fn save(out: &std::path::Path, name: &str, value: serde_json::Value) {
    let temporary = out.join(format!("{name}.tmp"));
    std::fs::write(&temporary, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    std::fs::rename(temporary, out.join(name)).unwrap();
}
async fn signal(out: &std::path::Path, name: &str) {
    let deadline = Instant::now() + Duration::from_secs(90);
    while !out.join(name).exists() {
        assert!(Instant::now() < deadline, "missing {name}");
        sleep(Duration::from_millis(20)).await;
    }
}
#[tokio::test]
#[ignore = "requires owned original MySQL/TC and raw auth recorder; run native_mysql_rsa_auth.py"]
async fn original_mysql_rsa_authentication_and_formal_at_callback() {
    tokio::time::timeout(Duration::from_secs(150), run())
        .await
        .unwrap();
}
async fn run() {
    let out = PathBuf::from(std::env::var("SEATA_AUTH_OUT").unwrap());
    let mode = std::env::var("SEATA_AUTH_MODE").unwrap();
    let user = if mode == "sha256" {
        "auth_sha256"
    } else {
        "auth_client"
    };
    let password = std::env::var("SEATA_MYSQL_PASSWORD").unwrap();
    let password = if mode == "bad_password" {
        format!("{password}-wrong")
    } else {
        password
    };
    let mut options = MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_AUTH_PORT").unwrap().parse().unwrap(),
        user.into(),
        password,
        "admission".into(),
    )
    .unwrap()
    .pool_limits(2, Duration::from_secs(3))
    .unwrap()
    .operation_timeout(Duration::from_secs(8))
    .unwrap()
    .with_java_tc_mysql_db_lock_profile()
    .require_at_jdbc_profile_binding();
    options = if mode == "tls" || mode == "wrong_ca" {
        options
            .with_tls_ca_pem(std::fs::read(std::env::var("SEATA_AUTH_CA").unwrap()).unwrap())
            .unwrap()
    } else {
        options.with_insecure_loopback_tls().unwrap()
    };
    let connected = MySqlDataSource::connect(options).await;
    if std::env::var("SEATA_AUTH_EXPECT_REJECT").unwrap() == "true" {
        assert!(
            connected.is_err(),
            "invalid authentication unexpectedly connected"
        );
        save(
            &out,
            "rejected.json",
            serde_json::json!({"mode":mode,"accepted":false}),
        );
        return;
    }
    let source = connected.unwrap();
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
        "mysql-rsa-auth".into(),
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
    .build()
    .unwrap();
    let tm = TransactionManager::builder(pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()), id)
        .request_timeout(Duration::from_secs(15))
        .build()
        .unwrap();
    let config = TransactionConfig::builder("mysql-rsa-auth".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut global = tm.begin(&config).await.unwrap();
    let xid = global.xid().clone();
    let mut local = source.begin().await.unwrap();
    assert_eq!(
        local
            .execute(
                &SqlRecognizerFactory::recognize("UPDATE auth_work SET v=v+1 WHERE id=1").unwrap(),
                &[]
            )
            .await
            .unwrap(),
        1
    );
    save(
        &out,
        "connection.json",
        serde_json::json!({"mode":mode,"xid":xid.as_str(),"resource":adapter.key().resource_id()}),
    );
    signal(&out, "connection-observed").await;
    let receipt = adapter
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap()
        .commit_changed_and_report(&adapter, &rm.client())
        .await
        .unwrap();
    let branch = receipt.identity().branch_id();
    let root_options = sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(
            std::env::var("SEATA_REAL_MYSQL_PORT")
                .unwrap()
                .parse()
                .unwrap(),
        )
        .username("root")
        .password(&std::env::var("SEATA_MYSQL_PASSWORD").unwrap())
        .database("admission")
        .charset("utf8mb4")
        .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled);
    let mut observer = sqlx::MySqlConnection::connect_with(&root_options)
        .await
        .unwrap();
    let witness:(Vec<u8>,Vec<u8>,u8)=sqlx::query_as("SELECT u.rollback_info,b.undo_sha256,b.first_phase_committed FROM undo_log u JOIN seata_rs_at_branch_v1 b ON b.xid=CAST(u.xid AS BINARY) AND b.branch_id=u.branch_id WHERE u.xid=? AND u.branch_id=? AND u.log_status=0").bind(xid.as_str()).bind(branch).fetch_one(&mut observer).await.unwrap();
    assert_eq!(Sha256::digest(&witness.0).to_vec(), witness.1);
    assert_eq!(witness.2, 1);
    std::fs::write(out.join("probe.undo.json"), &witness.0).unwrap();
    save(
        &out,
        "phase.json",
        serde_json::json!({"mode":mode,"xid":xid.as_str(),"branch_id":branch,"resource":adapter.key().resource_id(),"lock":receipt.lock_key(),"checksum_persisted":true}),
    );
    signal(&out, "release").await;
    let commit = mode == "caching";
    if commit {
        global.commit().await.unwrap();
    } else {
        global.rollback().await.unwrap();
    }
    let end = Instant::now() + Duration::from_secs(30);
    while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
        assert!(Instant::now() < end);
        sleep(Duration::from_millis(20)).await;
    }
    let value: i32 = sqlx::query_scalar("SELECT v FROM auth_work WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(value, if commit { 11 } else { 10 });
    save(
        &out,
        "finished.json",
        serde_json::json!({"mode":mode,"xid":xid.as_str(),"branch_id":branch,"value":value}),
    );
    signal(&out, "stop").await;
    tm.shutdown().await.unwrap();
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
