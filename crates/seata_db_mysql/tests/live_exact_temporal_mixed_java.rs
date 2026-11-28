//! 原 Java 与双 Rust 在精确 DATE/TIME 主键上的实际 TC 锁竞争。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_datasource::{FieldValue, InternalTime, LocalDate};
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::{
    v1::FrameLimits, GlobalStatus, Message, PayloadLimits, TransactionExceptionCode,
};
use seata_rm::{
    BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager, RmCallError,
};
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
    let deadline = Instant::now() + Duration::from_secs(100);
    while !out.join(name).exists() {
        assert!(
            Instant::now() < deadline,
            "controller signal missing: {name}"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
#[ignore = "requires owned original Java TC/RM and MySQL; run java_writer_jdbc_profile_mixed.py"]
async fn original_java_and_rust_share_exact_temporal_locks() {
    tokio::time::timeout(Duration::from_secs(240), run())
        .await
        .unwrap();
}

async fn run() {
    let out = PathBuf::from(std::env::var("SEATA_MIXED_TEMPORAL_OUT").unwrap());
    let version = std::env::var("SEATA_MIXED_RUST_VERSION").unwrap();
    let profile_code: u8 = std::env::var("SEATA_MIXED_TEMPORAL_PROFILE")
        .unwrap()
        .parse()
        .unwrap();
    assert!(profile_code == 5 || profile_code == 6);
    let mut admin = owned_mysql::admin().await;
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(database, std::env::var("SEATA_MYSQL_DATABASE").unwrap());
    let profile = MySqlJdbcProfile::from_storage_code(profile_code).unwrap();
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
    let resource = manager.key().resource_id().to_owned();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(manager.clone()).unwrap();
    let store = FileBranchStore::open(
        out.join("branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let identity = RegistrationIdentity::new(
        "1.1.0".into(),
        format!("mixed-temporal-{version}"),
        "cross_lock".into(),
        None,
    )
    .unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        identity.clone(),
        catalog,
        store,
    )
    .request_timeout(Duration::from_secs(15))
    .callback_timeout(Duration::from_secs(15))
    .build()
    .unwrap();
    let tm =
        TransactionManager::builder(pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()), identity)
            .request_timeout(Duration::from_secs(15))
            .build()
            .unwrap();
    let cfg = TransactionConfig::builder(format!("mixed-temporal-{version}"))
        .transaction_timeout(Duration::from_secs(90))
        .build()
        .unwrap();
    let cases = [
        (
            "date",
            "temporal_dates",
            "temporal_dates:1582-10-05",
            FieldValue::LocalDate(LocalDate::parse_sql("1582-10-05").unwrap()),
        ),
        (
            "time",
            "temporal_times",
            "temporal_times:-25:01:02.123456",
            FieldValue::InternalTime(InternalTime::parse_sql("-25:01:02.123456").unwrap()),
        ),
    ];
    let mut conflicts = Vec::new();
    for (name, table, lock, parameter) in &cases {
        let mut global = tm.begin(&cfg).await.unwrap();
        let xid = global.xid().clone();
        let mut local = source.begin().await.unwrap();
        let sql = SqlRecognizerFactory::recognize(&format!("UPDATE {table} SET v=v+1 WHERE k=?"))
            .unwrap();
        assert_eq!(
            local
                .execute(&sql, std::slice::from_ref(parameter))
                .await
                .unwrap(),
            1
        );
        let error = manager
            .prepare_transaction(local, &rm.client(), xid.as_str(), None)
            .await
            .unwrap_err();
        let MySqlPrepareError::Registration(RmCallError::DefiniteRejected { response, .. }) = error
        else {
            panic!("expected original TC rejection: {error:?}");
        };
        let Some(Message::BranchRegisterResponse(reply)) = response.body else {
            panic!("missing original response");
        };
        let code = reply.response.transaction_exception_code;
        assert!(matches!(
            code,
            TransactionExceptionCode::LockKeyConflict
                | TransactionExceptionCode::LockKeyConflictFailFast
        ));
        global.rollback().await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
            assert!(Instant::now() < deadline);
            sleep(Duration::from_millis(20)).await;
        }
        conflicts.push(serde_json::json!({"case":name,"xid":xid.as_str(),"lock":lock,"exception":code.to_java_v1()}));
    }
    save(
        &out,
        "conflicts.json",
        serde_json::json!({"version":version,"profile":profile_code,"resource":resource,"cases":conflicts}),
    );
    signal(&out, "java-released").await;
    for (name, table, lock, parameter) in &cases {
        let mut global = tm.begin(&cfg).await.unwrap();
        let xid = global.xid().clone();
        let mut local = source.begin().await.unwrap();
        let sql = SqlRecognizerFactory::recognize(&format!("UPDATE {table} SET v=v+1 WHERE k=?"))
            .unwrap();
        assert_eq!(
            local
                .execute(&sql, std::slice::from_ref(parameter))
                .await
                .unwrap(),
            1
        );
        let prepared = manager
            .prepare_transaction(local, &rm.client(), xid.as_str(), None)
            .await
            .unwrap();
        let receipt = prepared.registered_branch().unwrap();
        assert_eq!(receipt.lock_key(), Some(*lock));
        save(
            &out,
            &format!("admitted-{name}.json"),
            serde_json::json!({"case":name,"version":version,
            "profile":profile_code,"xid":xid.as_str(),"branch_id":receipt.identity().branch_id(),"resource":resource,"lock":lock}),
        );
        signal(&out, &format!("admitted-{name}.ack")).await;
        prepared.rollback().await.unwrap();
        global.rollback().await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
            assert!(Instant::now() < deadline);
            sleep(Duration::from_millis(20)).await;
        }
    }
    tm.shutdown().await.unwrap();
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
    save(
        &out,
        "finished.json",
        serde_json::json!({"version":version,"profile":profile_code,"conflicts":2,"admitted":2}),
    );
}
