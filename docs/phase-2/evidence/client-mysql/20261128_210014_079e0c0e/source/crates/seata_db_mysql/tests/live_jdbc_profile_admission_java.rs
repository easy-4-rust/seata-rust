//! 原 Java UTC EXACT_DATE_TIME 对照、精度、主键与实际 AT 二阶段；对应 Java InternalTime/ConnectionProxy。
#![cfg(feature = "experimental_changed_commit")]
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use seata_at::{JacksonUndoLogParser, MySqlJdbcProfile, SqlRecognizerFactory, SqlType, SqlUndoLog};
use seata_datasource::{FieldValue, InternalTime, LocalDate, TableRecords};
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
    let rows=sqlx::query("SELECT id,CAST(v AS CHAR),CAST(d0 AS CHAR),CAST(d1 AS CHAR),CAST(d2 AS CHAR),CAST(d3 AS CHAR),CAST(d4 AS CHAR),CAST(d5 AS CHAR) FROM undo_jdbc_profile ORDER BY id").fetch_all(admin).await.unwrap();
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
    sqlx::query_as("SELECT CAST(k AS CHAR),v FROM undo_jdbc_profile_keys ORDER BY k")
        .fetch_all(admin)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires owned original Java TC and MySQL; run jdbc_profile_admission_java.py"]
async fn original_java_tc_jdbc_profile_admission() {
    tokio::time::timeout(Duration::from_secs(180), run())
        .await
        .unwrap();
}
async fn run() {
    let out = PathBuf::from(std::env::var("SEATA_AT_JDBC_PROFILE_OUT").unwrap());
    let commit = std::env::var("SEATA_AT_JDBC_PROFILE_DIRECTION").unwrap() == "commit";
    let mode = std::env::var("SEATA_AT_JDBC_PROFILE_PROFILE").unwrap_or("boolean".into());
    let profile = if mode == "numeric" {
        MySqlJdbcProfile::Connector8NumericTinyIntExactTemporalUtc
    } else {
        MySqlJdbcProfile::Connector8ExactTemporalUtc
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
        database.starts_with("undo_jdbc_profile_")
            && database == std::env::var("SEATA_MYSQL_DATABASE").unwrap()
    );
    for table in [
        "undo_jdbc_profile_zero",
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
        include_str!("../migrations/011_seata_rs_at_tinyint_profile_binding.sql"),
        include_str!("../migrations/021_seata_rs_at_jdbc_profile_binding.sql"),
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
    let owner = std::env::var("SEATA_JDBC_PROFILE_GUARD_OWNER").unwrap();
    assert!(owner == "transaction" || owner == "manager");
    let options = owned_mysql::options(None)
        .with_java_tc_mysql_db_lock_profile()
        .pool_limits(8, Duration::from_secs(15))
        .unwrap()
        .operation_timeout(Duration::from_secs(30))
        .unwrap();
    let ds = MySqlDataSource::connect(if owner == "transaction" {
        options.clone().require_at_jdbc_profile_binding()
    } else {
        options.clone()
    })
    .await
    .unwrap();
    let manager_source = MySqlDataSource::connect(if owner == "manager" {
        options.require_at_jdbc_profile_binding()
    } else {
        options
    })
    .await
    .unwrap();
    let adapter = Arc::new(MySqlDataSourceManager::new(manager_source.clone(), profile).unwrap());
    let key = adapter.key().clone();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding(slot,resource_id,tinyint1_is_bit,jdbc_profile) VALUES(1,?,?,?)")
        .bind(key.resource_id().as_bytes())
        .bind(u8::from(profile.boolean_tinyint()))
        .bind(profile.storage_code())
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
    let tm = TransactionManager::builder(
        pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
        registration(),
    )
    .request_timeout(Duration::from_secs(30))
    .build()
    .unwrap();
    let cfg = TransactionConfig::builder("native-exact_date_time-utc".into())
        .transaction_timeout(Duration::from_secs(90))
        .build()
        .unwrap();
    let mut global = tm.begin(&cfg).await.unwrap();
    let xid = global.xid().clone();
    sqlx::query("DROP TABLE IF EXISTS undo_jdbc_profile_probe")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE undo_jdbc_profile_probe(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::query("INSERT INTO undo_jdbc_profile_probe VALUES(1,0)")
        .execute(&mut admin)
        .await
        .unwrap();
    let mut rejected = Vec::new();
    for code in 1..=6 {
        if code == profile.storage_code() {
            continue;
        }
        let wrong_profile = MySqlJdbcProfile::from_storage_code(code).unwrap();
        let wrong = MySqlDataSourceManager::new(manager_source.clone(), wrong_profile).unwrap();
        let mut candidate = ds.begin().await.unwrap();
        assert_eq!(
            candidate
                .execute(
                    &SqlRecognizerFactory::recognize(
                        "UPDATE undo_jdbc_profile_probe SET v=? WHERE id=1"
                    )
                    .unwrap(),
                    &[FieldValue::Signed(999)]
                )
                .await
                .unwrap(),
            1
        );
        let error = wrong
            .prepare_transaction(candidate, &rm.client(), xid.as_str(), None)
            .await
            .unwrap_err();
        let reason = if wrong_profile.boolean_tinyint() == profile.boolean_tinyint() {
            "MySQL AT JDBC profile binding is missing or differs"
        } else {
            "MySQL AT TINYINT(1) profile binding is missing or differs"
        };
        assert!(
            matches!(error,seata_db_mysql::MySqlPrepareError::Local{source:DbError::Input{reason:actual}} if actual==reason)
        );
        let value: i32 = sqlx::query_scalar("SELECT v FROM undo_jdbc_profile_probe WHERE id=1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(value, 0);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM branch_table")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(count, 0);
        rejected.push(serde_json::json!({"profile":code,"same_tinyint":wrong_profile.boolean_tinyint()==profile.boolean_tinyint(),"reason":reason,"business_rolled_back":true,"tc_branches":count}));
    }
    save(
        &out,
        "profile-admission.json",
        serde_json::json!({"owner":owner,"profile":profile.storage_code(),"rejected":rejected}),
    );
    let mut local = ds.begin().await.unwrap();
    for table in ["undo_jdbc_profile", "undo_jdbc_profile_keys"] {
        let records = local
            .before_image(
                &SqlRecognizerFactory::recognize(&format!("UPDATE {table} SET v=?")).unwrap(),
                &[FieldValue::Null],
            )
            .await
            .expect("native EXACT_DATE_TIME metadata and image must be supported");
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
    let new_value = InternalTime::parse_sql("-838:59:58.123456").unwrap();
    let inserted = InternalTime::parse_sql("-25:01:02.000001").unwrap();
    let min = LocalDate::parse_sql("1582-10-14").unwrap();
    let epoch = LocalDate::parse_sql("1582-10-05").unwrap();
    let end = LocalDate::parse_sql("9999-12-31").unwrap();
    for (sql, params) in [
        (
            "UPDATE undo_jdbc_profile SET v=? WHERE id=1",
            vec![FieldValue::InternalTime(new_value)],
        ),
        (
            "INSERT INTO undo_jdbc_profile(id,v) VALUES(10001,?)",
            vec![FieldValue::InternalTime(inserted)],
        ),
        ("DELETE FROM undo_jdbc_profile WHERE id=20001", vec![]),
        (
            "UPDATE undo_jdbc_profile_keys SET v=8 WHERE k=?",
            vec![FieldValue::LocalDate(epoch)],
        ),
        (
            "INSERT INTO undo_jdbc_profile_keys(k,v) VALUES(?,10)",
            vec![FieldValue::LocalDate(min)],
        ),
        (
            "DELETE FROM undo_jdbc_profile_keys WHERE k=?",
            vec![FieldValue::LocalDate(end)],
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
    let observation:(u8,u8,u8,i64,i64,i64)=sqlx::query_as("SELECT (SELECT jdbc_profile FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?),(SELECT jdbc_profile FROM seata_rs_at_commit_attempt WHERE xid=? AND branch_id=?),(SELECT phase FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?),(SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0),(SELECT COUNT(*) FROM lock_table WHERE xid=? AND branch_id=?),(SELECT COUNT(*) FROM seata_rs_at_branch_v1 b JOIN undo_log u ON b.xid=CAST(u.xid AS BINARY) AND b.branch_id=u.branch_id WHERE u.xid=? AND u.branch_id=? AND b.undo_sha256=UNHEX(SHA2(u.rollback_info,256)) AND b.first_phase_committed=1)").bind(xid.as_str().as_bytes()).bind(branch).bind(xid.as_str().as_bytes()).bind(branch).bind(xid.as_str().as_bytes()).bind(branch).bind(xid.as_str()).bind(branch).bind(xid.as_str()).bind(branch).bind(xid.as_str()).bind(branch).fetch_one(&mut admin).await.unwrap();
    assert_eq!(
        observation,
        (profile.storage_code(), profile.storage_code(), 0, 1, 6, 1)
    );
    save(
        &out,
        "before-end.json",
        serde_json::json!({"xid":xid.as_str(),"branch_id":branch,"profile":profile.storage_code(),"wrong_profile_rejected":true,"counts":[observation.0,observation.1,observation.2,observation.3,observation.4,observation.5]}),
    );
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
        expected[0][1] = serde_json::json!("-838:59:58.123456");
        expected.push(serde_json::json!([
            10001,
            "-25:01:02.000001",
            null,
            null,
            null,
            null,
            null,
            null
        ]));
        expected.sort_by_key(|r| r[0].as_i64().unwrap());
        expected_keys.retain(|(k, _)| k != "9999-12-31");
        expected_keys
            .iter_mut()
            .find(|(k, _)| k == "1582-10-05")
            .unwrap()
            .1 = 8;
        expected_keys.push(("1582-10-14".into(), 10));
        expected_keys.sort();
    }
    assert_eq!(actual, expected);
    assert_eq!(actual_keys, expected_keys);
    save(&out, "after-business.json", serde_json::json!(actual));
    save(&out, "after-keys.json", serde_json::json!(actual_keys));
    let insert =
        SqlRecognizerFactory::recognize("INSERT INTO undo_jdbc_profile(id,v) VALUES(999,?)")
            .unwrap();
    for text in [
        "838:59:59.000001",
        "-838:59:59.000001",
        "839:00:00",
        "-839:00:00",
    ] {
        let mut outside = ds.begin().await.unwrap();
        let observed = outside
            .execute(&insert, &[FieldValue::Text(text.into())])
            .await;
        assert!(
            matches!(
                observed,
                Err(DbError::Driver {
                    code: Some(1292),
                    ..
                })
            ),
            "{text}: {observed:?}"
        );
        assert!(matches!(
            outside.execute(&insert, &[FieldValue::Null]).await,
            Err(DbError::Poisoned)
        ));
        outside.rollback().await.unwrap();
    }
    sqlx::query(
        "CREATE TABLE undo_jdbc_profile_zero(id INT PRIMARY KEY,v DATE NULL) ENGINE=InnoDB",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::query("SET SESSION sql_mode=''")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("INSERT INTO undo_jdbc_profile_zero VALUES(1,'0000-00-00')")
        .execute(&mut admin)
        .await
        .unwrap();
    let mut zero = ds.begin().await.unwrap();
    assert!(matches!(
        zero.before_image(
            &SqlRecognizerFactory::recognize("UPDATE undo_jdbc_profile_zero SET v=?").unwrap(),
            &[FieldValue::Null]
        )
        .await,
        Err(DbError::Image(seata_datasource::ImageError::Invalid { .. }))
    ));
    zero.rollback().await.unwrap();
    save(
        &out,
        "negative-checks.json",
        serde_json::json!({"time_overflow_mysql_code":1292,"poisoned":true,"zero_date_rejected_not_null":true,"old_profile_rejected":true,"outside_range_rejected":4}),
    );
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
    manager_source.close().await.unwrap();
    save(
        &out,
        "result.json",
        serde_json::json!({"status":"AT_JDBC_PROFILE_ADMISSION_VERIFIED","xid":xid.as_str(),"branch_id":branch,"direction":if commit {"commit"}else{"rollback"},"profile":profile.storage_code(),"observed_end_status":format!("{status:?}"),"journal":"Completed","business_statements":6}),
    );
}
