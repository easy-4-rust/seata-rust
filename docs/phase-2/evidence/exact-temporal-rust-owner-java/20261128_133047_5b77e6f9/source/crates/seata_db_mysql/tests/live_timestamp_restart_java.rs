//! 原 Java UTC TIMESTAMP 对照、精度、主键与实际 AT 二阶段；对应 Java Timestamp/ConnectionProxy。
#![cfg(feature = "experimental_changed_commit")]
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/timestamp_owned_process.rs"]
mod timestamp_owned_process;
use seata_at::{JacksonUndoLogParser, MySqlJdbcProfile, SqlRecognizerFactory, SqlType, SqlUndoLog};
use seata_datasource::{FieldValue, TableRecords, Timestamp};
use seata_db_mysql::{DbError, MySqlDataSource, MySqlDataSourceManager};
use seata_protocol::{v1::FrameLimits, GlobalStatus, PayloadLimits};
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
use timestamp_owned_process::TimestampOwnedProcess;
use tokio::time::Instant;
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

fn registration() -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        "phase0-auth".into(),
        "default_tx_group".into(),
        Some("phase0-local-only".into()),
    )
    .unwrap()
}

fn save(out: &std::path::Path, name: &str, value: serde_json::Value) {
    std::fs::write(out.join(name), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}
async fn business(admin: &mut sqlx::MySqlConnection) -> Vec<serde_json::Value> {
    let rows=sqlx::query("SELECT id,CAST(v AS CHAR),CAST(d0 AS CHAR),CAST(d1 AS CHAR),CAST(d2 AS CHAR),CAST(d3 AS CHAR),CAST(d4 AS CHAR),CAST(d5 AS CHAR) FROM undo_timestamp ORDER BY id").fetch_all(admin).await.unwrap();
    rows.iter()
        .map(|r| {
            let mut values = vec![serde_json::json!(r.get::<i32, _>(0))];
            for i in 1..8 {
                values.push(serde_json::json!(r.get::<Option<String>, _>(i)));
            }
            serde_json::Value::Array(values)
        })
        .collect()
}
async fn keys(admin: &mut sqlx::MySqlConnection) -> Vec<(String, i32)> {
    sqlx::query_as("SELECT CAST(k AS CHAR),v FROM undo_timestamp_keys ORDER BY k")
        .fetch_all(admin)
        .await
        .unwrap()
}
fn selected_profile() -> MySqlJdbcProfile {
    if std::env::var("SEATA_AT_TIMESTAMP_PROFILE").unwrap() == "numeric" {
        MySqlJdbcProfile::Connector8NumericTinyIntUtc
    } else {
        MySqlJdbcProfile::Connector8DefaultUtc
    }
}
async fn source() -> MySqlDataSource {
    MySqlDataSource::connect(
        owned_mysql::options(None)
            .with_java_tc_mysql_db_lock_profile()
            .pool_limits(8, Duration::from_secs(15))
            .unwrap()
            .operation_timeout(Duration::from_secs(30))
            .unwrap(),
    )
    .await
    .unwrap()
}
fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn spawn(out: &std::path::Path, test: &str, xid: &str, label: &str) -> TimestampOwnedProcess {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args([test, "--ignored", "--exact", "--nocapture"])
        .env("SEATA_TIMESTAMP_RESTART_XID", xid)
        .stdout(std::fs::File::create(out.join(format!("{label}.stdout"))).unwrap())
        .stderr(std::fs::File::create(out.join(format!("{label}.stderr"))).unwrap());
    let child = TimestampOwnedProcess::spawn(cmd);
    save(
        out,
        &format!("{label}-spawn.json"),
        serde_json::json!({"pid":child.pid(),"parent_pid":std::process::id(),"test":test}),
    );
    child
}
async fn wait_ready(path: &std::path::Path, child: &mut TimestampOwnedProcess) {
    let until = Instant::now() + Duration::from_secs(60);
    while std::fs::read(path)
        .ok()
        .is_none_or(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).is_err())
    {
        assert!(
            child.alive(),
            "owned process exited before checkpoint: {}",
            child.pid()
        );
        assert!(Instant::now() < until, "owned process checkpoint deadline");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(child.alive());
}
#[tokio::test]
#[ignore = "requires owned original Java TC and MySQL; run timestamp_restart_java.py"]
async fn original_java_tc_timestamp_restart() {
    tokio::time::timeout(Duration::from_secs(200), run_restart())
        .await
        .unwrap();
}
#[tokio::test]
#[ignore = "owned producer process only"]
async fn timestamp_restart_producer() {
    run_producer().await;
}
#[tokio::test]
#[ignore = "owned replacement RM process only"]
async fn timestamp_restart_worker() {
    tokio::time::timeout(Duration::from_secs(120), run_worker())
        .await
        .unwrap();
}
async fn run_worker() {
    let out = PathBuf::from(std::env::var("SEATA_AT_TIMESTAMP_OUT").unwrap());
    let before = read_json(&out.join("before-end.json"));
    let xid = std::env::var("SEATA_TIMESTAMP_RESTART_XID").unwrap();
    let branch = before["branch_id"].as_i64().unwrap();
    let profile = selected_profile();
    assert_eq!(before["xid"], xid);
    assert_eq!(before["profile"], profile.storage_code());
    let ds = source().await;
    let adapter = Arc::new(MySqlDataSourceManager::new(ds.clone(), profile).unwrap());
    let attempt = adapter
        .load_commit_attempt(&xid, branch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attempt.profile(), profile);
    assert!(adapter.commit_attempt_observed(&attempt).await.unwrap());
    let identity = BranchIdentity::new(xid.clone(), branch, adapter.key().clone(), None).unwrap();
    let store = FileBranchStore::open(
        out.join("branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(15))
    .callback_timeout(Duration::from_secs(30))
    .build()
    .unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    while rm.connection_snapshot().ready() == 0 {
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    save(
        &out,
        "worker-ready.json",
        serde_json::json!({"pid":std::process::id(),"xid":xid,"branch_id":branch,"profile":profile.storage_code(),"attempt_loaded":true,"witness_observed":true,"registered":true}),
    );
    let commit = std::env::var("SEATA_AT_TIMESTAMP_DIRECTION").unwrap() == "commit";
    let mut admin = owned_mysql::admin().await;
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        let terminal:(u8,u8,u8,u8)=sqlx::query_as("SELECT phase,terminal,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?").bind(xid.as_bytes()).bind(branch).fetch_one(&mut admin).await.unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM global_table WHERE xid=?")
            .bind(&xid)
            .fetch_one(&mut admin)
            .await
            .unwrap();
        if terminal == if commit { (1, 0, 1, 1) } else { (2, 1, 0, 0) } && remaining == 0 {
            break;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        store
            .lease(
                identity,
                if commit {
                    BranchPhase::Commit
                } else {
                    BranchPhase::Rollback
                }
            )
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    rm.shutdown().await.unwrap();
    ds.close().await.unwrap();
    save(
        &out,
        "worker-result.json",
        serde_json::json!({"pid":std::process::id(),"xid":xid,"branch_id":branch,"profile":profile.storage_code(),"journal":"Completed","fresh_process":true}),
    );
}
async fn run_producer() {
    let out = PathBuf::from(std::env::var("SEATA_AT_TIMESTAMP_OUT").unwrap());
    let commit = std::env::var("SEATA_AT_TIMESTAMP_DIRECTION").unwrap() == "commit";
    let mode = std::env::var("SEATA_AT_TIMESTAMP_PROFILE").unwrap_or("boolean".into());
    let profile = if mode == "numeric" {
        MySqlJdbcProfile::Connector8NumericTinyIntUtc
    } else {
        MySqlJdbcProfile::Connector8DefaultUtc
    };
    let legacy = if mode == "numeric" {
        MySqlJdbcProfile::Connector8NumericTinyInt
    } else {
        MySqlJdbcProfile::Connector8Default
    };
    let mut admin = owned_mysql::admin().await;
    let database: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert!(
        database.starts_with("undo_timestamp_")
            && database == std::env::var("SEATA_MYSQL_DATABASE").unwrap()
    );
    for table in [
        "undo_timestamp_zero",
        "undo_log",
        "seata_rs_at_resource_binding",
        "seata_rs_at_commit_attempt",
        "seata_rs_at_branch_v1",
    ] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(&mut admin)
            .await
            .unwrap();
    }
    sqlx::raw_sql(
        &std::fs::read_to_string(std::env::var("SEATA_UNDO_SCHEMA_PATH").unwrap()).unwrap(),
    )
    .execute(&mut admin)
    .await
    .unwrap();
    for migration in [
        include_str!("../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/005_seata_rs_at_resource_binding.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
        include_str!("../migrations/013_seata_rs_at_undo_checksum.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    let original = business(&mut admin).await;
    let original_keys = keys(&mut admin).await;
    save(&out, "before-business.json", serde_json::json!(original));
    save(&out, "before-keys.json", serde_json::json!(original_keys));
    let ds = MySqlDataSource::connect(
        owned_mysql::options(None)
            .with_java_tc_mysql_db_lock_profile()
            .pool_limits(8, Duration::from_secs(15))
            .unwrap()
            .operation_timeout(Duration::from_secs(30))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = Arc::new(MySqlDataSourceManager::new(ds.clone(), profile).unwrap());
    let key = adapter.key().clone();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(key.resource_id().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let store = FileBranchStore::open(
        out.join("branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(15))
    .callback_timeout(Duration::from_secs(30))
    .build()
    .unwrap();
    let xid = std::env::var("SEATA_TIMESTAMP_RESTART_XID").unwrap();
    let mut local = ds.begin().await.unwrap();
    for table in ["undo_timestamp", "undo_timestamp_keys"] {
        let records = local
            .before_image(
                &SqlRecognizerFactory::recognize(&format!("UPDATE {table} SET v=?")).unwrap(),
                &[FieldValue::Null],
            )
            .await
            .expect("native TIMESTAMP metadata and image must be supported");
        let meta = records.table_meta().clone();
        let logs = [
            SqlUndoLog::new(
                SqlType::Insert,
                TableRecords::new(meta.clone()),
                records.clone(),
            )
            .unwrap(),
            SqlUndoLog::new(SqlType::Update, records.clone(), records.clone()).unwrap(),
            SqlUndoLog::new(SqlType::Delete, records, TableRecords::new(meta)).unwrap(),
        ];
        let bytes = JacksonUndoLogParser::default()
            .encode_prepared("127.0.0.1:8091:42", i64::MAX, &logs, profile)
            .unwrap();
        std::fs::write(out.join(format!("rust-java-{table}.json")), bytes).unwrap();
    }
    let new_value = Timestamp::parse_sql_utc("2038-01-19 03:14:07.123456").unwrap();
    let inserted = Timestamp::parse_sql_utc("2000-02-29 12:34:56.000001").unwrap();
    let min = Timestamp::parse_sql_utc("1970-01-01 00:00:01.000002").unwrap();
    let epoch = Timestamp::parse_sql_utc("1970-01-01 00:00:01.000001").unwrap();
    let end = Timestamp::parse_sql_utc("2038-01-19 03:14:07.123456").unwrap();
    for (sql, params) in [
        (
            "UPDATE undo_timestamp SET v=? WHERE id=1",
            vec![FieldValue::Timestamp(new_value)],
        ),
        (
            "INSERT INTO undo_timestamp(id,v) VALUES(10001,?)",
            vec![FieldValue::Timestamp(inserted)],
        ),
        ("DELETE FROM undo_timestamp WHERE id=20001", vec![]),
        (
            "UPDATE undo_timestamp_keys SET v=8 WHERE k=?",
            vec![FieldValue::Timestamp(epoch)],
        ),
        (
            "INSERT INTO undo_timestamp_keys(k,v) VALUES(?,10)",
            vec![FieldValue::Timestamp(min)],
        ),
        (
            "DELETE FROM undo_timestamp_keys WHERE k=?",
            vec![FieldValue::Timestamp(end)],
        ),
    ] {
        assert_eq!(
            local
                .execute(&SqlRecognizerFactory::recognize(sql).unwrap(), &params)
                .await
                .unwrap(),
            1
        );
    }
    let checked = adapter
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    let branch = checked.registered_branch().unwrap().identity().branch_id();
    checked
        .commit_changed_and_report(&adapter, &rm.client())
        .await
        .unwrap();
    let attempt = adapter
        .load_commit_attempt(xid.as_str(), branch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attempt.profile(), profile);
    assert!(adapter.commit_attempt_observed(&attempt).await.unwrap());
    let identity = BranchIdentity::new(xid.as_str().into(), branch, key.clone(), None).unwrap();
    let wrong = MySqlDataSourceManager::new(ds.clone(), legacy).unwrap();
    assert!(matches!(
        wrong
            .recover(
                &identity,
                if commit {
                    BranchPhase::Commit
                } else {
                    BranchPhase::Rollback
                }
            )
            .await,
        Err(seata_rm::RmError::IdentityConflict)
    ));
    assert!(matches!(
        wrong.load_commit_attempt(xid.as_str(), branch).await,
        Err(DbError::RecoveryConflict)
    ));
    let observation:(u8,u8,u8,i64,i64,i64)=sqlx::query_as("SELECT (SELECT jdbc_profile FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?),(SELECT jdbc_profile FROM seata_rs_at_commit_attempt WHERE xid=? AND branch_id=?),(SELECT phase FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?),(SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0),(SELECT COUNT(*) FROM lock_table WHERE xid=? AND branch_id=?),(SELECT COUNT(*) FROM seata_rs_at_branch_v1 b JOIN undo_log u ON b.xid=CAST(u.xid AS BINARY) AND b.branch_id=u.branch_id WHERE u.xid=? AND u.branch_id=? AND b.undo_sha256=UNHEX(SHA2(u.rollback_info,256)) AND b.first_phase_committed=1)").bind(xid.as_bytes()).bind(branch).bind(xid.as_bytes()).bind(branch).bind(xid.as_bytes()).bind(branch).bind(xid.as_str()).bind(branch).bind(xid.as_str()).bind(branch).bind(xid.as_str()).bind(branch).fetch_one(&mut admin).await.unwrap();
    assert_eq!(
        observation,
        (profile.storage_code(), profile.storage_code(), 0, 1, 6, 1)
    );
    save(
        &out,
        "before-end.json",
        serde_json::json!({"xid":xid.as_str(),"branch_id":branch,"profile":profile.storage_code(),"wrong_profile_rejected":true,"counts":[observation.0,observation.1,observation.2,observation.3,observation.4,observation.5]}),
    );
    save(
        &out,
        "producer-ready.json",
        serde_json::json!({"pid":std::process::id(),"xid":xid,"branch_id":branch,"profile":profile.storage_code()}),
    );
    loop {
        std::hint::black_box((&rm, &store, &ds));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
async fn run_restart() {
    let out = PathBuf::from(std::env::var("SEATA_AT_TIMESTAMP_OUT").unwrap());
    let commit = std::env::var("SEATA_AT_TIMESTAMP_DIRECTION").unwrap() == "commit";
    let profile = selected_profile();
    let tm = TransactionManager::builder(
        pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
        registration(),
    )
    .request_timeout(Duration::from_secs(30))
    .build()
    .unwrap();
    let cfg = TransactionConfig::builder("timestamp-process-restart".into())
        .transaction_timeout(Duration::from_secs(180))
        .build()
        .unwrap();
    let mut global = tm.begin(&cfg).await.unwrap();
    let xid = global.xid().clone();
    let mut producer = spawn(&out, "timestamp_restart_producer", xid.as_str(), "producer");
    wait_ready(&out.join("producer-ready.json"), &mut producer).await;
    wait_ready(&out.join("allow-producer-kill.json"), &mut producer).await;
    let produced = read_json(&out.join("producer-ready.json"));
    assert_eq!(produced["pid"], producer.pid());
    assert_eq!(produced["xid"], xid.as_str());
    let branch = produced["branch_id"].as_i64().unwrap();
    let crash = producer.crash();
    assert!(!producer.alive());
    let mut recovery = spawn(&out, "timestamp_restart_worker", xid.as_str(), "worker");
    assert_ne!(producer.pid(), recovery.pid());
    wait_ready(&out.join("worker-ready.json"), &mut recovery).await;
    wait_ready(&out.join("allow-worker-use.json"), &mut recovery).await;
    let ready = read_json(&out.join("worker-ready.json"));
    assert_eq!(ready["pid"], recovery.pid());
    assert_eq!(ready["xid"], xid.as_str());
    assert_eq!(ready["branch_id"], branch);
    let mut admin = owned_mysql::admin().await;
    let original: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(out.join("before-business.json")).unwrap()).unwrap();
    let original_keys: Vec<(String, i32)> =
        serde_json::from_slice(&std::fs::read(out.join("before-keys.json")).unwrap()).unwrap();
    let completion = if commit {
        global.commit().await
    } else {
        global.rollback().await
    }
    .unwrap();
    let status = match completion {
        TransactionCompletion::Observed { outcome, .. } => outcome.status(),
        other => panic!("{other:?}"),
    };
    assert!(if commit {
        status == GlobalStatus::Committed
    } else {
        matches!(
            status,
            GlobalStatus::Rollbacked | GlobalStatus::RollbackRetrying
        )
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let terminal:(u8,u8,u8,u8)=sqlx::query_as("SELECT phase,terminal,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?").bind(xid.as_str().as_bytes()).bind(branch).fetch_one(&mut admin).await.unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM global_table WHERE xid=?")
            .bind(xid.as_str())
            .fetch_one(&mut admin)
            .await
            .unwrap();
        if terminal == if commit { (1, 0, 1, 1) } else { (2, 1, 0, 0) } && remaining == 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let until = Instant::now() + Duration::from_secs(45);
    let worker_status = loop {
        if let Some(status) = recovery.status() {
            break status;
        }
        assert!(Instant::now() < until, "replacement RM must finish");
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert!(worker_status.success());
    let finished = read_json(&out.join("worker-result.json"));
    assert_eq!(finished["pid"], recovery.pid());
    let ds = source().await;
    let adapter = MySqlDataSourceManager::new(ds.clone(), profile).unwrap();
    let identity =
        BranchIdentity::new(xid.as_str().into(), branch, adapter.key().clone(), None).unwrap();
    let store = FileBranchStore::open(
        out.join("branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    save(
        &out,
        "process-proof.json",
        serde_json::json!({"parent_pid":std::process::id(),"producer_pid":producer.pid(),"worker_pid":recovery.pid(),"producer_forced_termination":true,"producer_exit_success":crash.success(),"producer_exit_code":crash.code(),"worker_exit_success":worker_status.success(),"xid":xid.as_str(),"branch_id":branch,"same_branch":true}),
    );
    assert_eq!(
        store
            .lease(
                identity,
                if commit {
                    BranchPhase::Commit
                } else {
                    BranchPhase::Rollback
                }
            )
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    let actual = business(&mut admin).await;
    let actual_keys = keys(&mut admin).await;
    let mut expected = original.clone();
    let mut expected_keys = original_keys.clone();
    if commit {
        expected.retain(|r| r[0] != 20001);
        expected[0][1] = serde_json::json!("2038-01-19 03:14:07.123456");
        expected.push(serde_json::json!([
            10001,
            "2000-02-29 12:34:56.000001",
            null,
            null,
            null,
            null,
            null,
            null
        ]));
        expected.sort_by_key(|r| r[0].as_i64().unwrap());
        expected_keys.retain(|(k, _)| k != "2038-01-19 03:14:07.123456");
        expected_keys
            .iter_mut()
            .find(|(k, _)| k == "1970-01-01 00:00:01.000001")
            .unwrap()
            .1 = 8;
        expected_keys.push(("1970-01-01 00:00:01.000002".into(), 10));
        expected_keys.sort();
    }
    assert_eq!(actual, expected);
    assert_eq!(actual_keys, expected_keys);
    save(&out, "after-business.json", serde_json::json!(actual));
    save(&out, "after-keys.json", serde_json::json!(actual_keys));
    let mut invalid = ds.begin().await.unwrap();
    let insert =
        SqlRecognizerFactory::recognize("INSERT INTO undo_timestamp(id,v) VALUES(999,?)").unwrap();
    assert!(matches!(
        invalid
            .execute(&insert, &[FieldValue::Text("2001-02-30 00:00:00".into())])
            .await,
        Err(DbError::Driver {
            code: Some(1292),
            ..
        })
    ));
    assert!(matches!(
        invalid.execute(&insert, &[FieldValue::Null]).await,
        Err(DbError::Poisoned)
    ));
    invalid.rollback().await.unwrap();
    for text in [
        "1970-01-01 00:00:00",
        "1970-01-01 00:00:00.000001",
        "2038-01-19 03:14:08",
        "1000-01-01 00:00:00",
    ] {
        let mut outside = ds.begin().await.unwrap();
        assert!(
            matches!(
                outside
                    .execute(&insert, &[FieldValue::Text(text.into())])
                    .await,
                Err(DbError::Driver {
                    code: Some(1292),
                    ..
                })
            ),
            "MySQL must reject TIMESTAMP outside range: {text}"
        );
        outside.rollback().await.unwrap();
    }
    sqlx::query(
        "CREATE TABLE undo_timestamp_zero(id INT PRIMARY KEY,v TIMESTAMP(6) NULL) ENGINE=InnoDB",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::query("SET SESSION sql_mode=''")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("INSERT INTO undo_timestamp_zero VALUES(1,'0000-00-00 00:00:00')")
        .execute(&mut admin)
        .await
        .unwrap();
    let mut zero = ds.begin().await.unwrap();
    assert!(matches!(
        zero.before_image(
            &SqlRecognizerFactory::recognize("UPDATE undo_timestamp_zero SET v=?").unwrap(),
            &[FieldValue::Null]
        )
        .await,
        Err(DbError::Image(seata_datasource::ImageError::Invalid { .. }))
    ));
    zero.rollback().await.unwrap();
    save(
        &out,
        "negative-checks.json",
        serde_json::json!({"invalid_date_mysql_code":1292,"poisoned":true,"zero_date_rejected_not_null":true,"legacy_profile_rejected":true,"outside_range_rejected":4}),
    );
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
    save(
        &out,
        "result.json",
        serde_json::json!({"status":"AT_TIMESTAMP_COLUMN_VERIFIED","xid":xid.as_str(),"branch_id":branch,"direction":if commit {"commit"}else{"rollback"},"profile":profile.storage_code(),"observed_end_status":format!("{status:?}"),"journal":"Completed","business_statements":6}),
    );
}
