//! 正式 RM 恢复行约束：只在运行器自有 MySQL 实例执行。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
pub(crate) mod fixture;
#[path = "support/owned_mysql.rs"]
pub(crate) mod owned_mysql;
#[path = "support/rm_recovery_fixture.rs"]
pub(crate) mod recovery;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
pub(crate) mod rm_fixture;
#[path = "support/undo_rollback_fixture.rs"]
pub(crate) mod rollback_fixture;
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::{DbError, MySqlDataSource};
use seata_rm::{BranchIdentity, BranchPhase, BranchResolution, BranchResource};

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn recovery_table_is_reserved_before_any_business_dml() {
    let mut admin = owned_mysql::admin().await;
    sqlx::query("DROP TABLE IF EXISTS seata_rs_at_branch_v1")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE seata_rs_at_branch_v1(id INT PRIMARY KEY,v INT) ENGINE=InnoDB")
        .execute(&mut admin)
        .await
        .unwrap();
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let value = tx
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO seata_rs_at_branch_v1(id,v) VALUES(1,2)")
                .unwrap(),
            &[],
        )
        .await;
    assert!(
        matches!(value, Err(DbError::Unsupported { .. })),
        "reserved recovery table must be rejected before business DML"
    );
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn compensation_and_terminal_are_atomic_and_duplicate_callbacks_preserve_later_business() {
    let (mut admin, ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        seata_at::MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    recovery::schema(&mut admin).await;
    let adapter = recovery::adapter(ds);
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
    assert_eq!(
        adapter
            .apply(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    sqlx::query("INSERT INTO undo_work VALUES(1,'later-business')")
        .execute(&mut admin)
        .await
        .unwrap();
    for _ in 0..3 {
        assert_eq!(
            adapter
                .apply(receipt.identity(), BranchPhase::Rollback)
                .await
                .unwrap(),
            BranchResolution::Applied
        );
        assert_eq!(
            adapter
                .recover(receipt.identity(), BranchPhase::Rollback)
                .await
                .unwrap(),
            BranchResolution::Applied
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT v FROM undo_work WHERE id=1")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        "later-business"
    );
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn absent_enrollment_is_unknown_but_enrolled_missing_undo_fences_and_finishes_atomically() {
    let (mut admin, adapter) = recovery::empty().await;
    let receipt = recovery::receipt(adapter.clone(), None).await;
    assert_eq!(
        adapter
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    adapter.enroll(&receipt).await.unwrap();
    sqlx::query(
        "INSERT INTO undo_log VALUES(101,'rollback:xid','branchId=42',X'0102',0,now(6),now(6))",
    )
    .execute(&mut admin)
    .await
    .unwrap();
    let orphan = rollback_fixture::snapshot(&mut admin).await;
    assert_eq!(
        adapter
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 0));
    assert_eq!(orphan, rollback_fixture::snapshot(&mut admin).await);
    // 清理自有夹具的孤儿片后，同一分支才能按无 Undo 路径收敛。
    sqlx::query("DELETE FROM undo_log WHERE xid='rollback:xid' AND branch_id=101")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        adapter
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT log_status FROM undo_log WHERE xid='rollback:xid' AND branch_id=42"
        )
        .fetch_one(&mut admin)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn dirty_failure_keeps_undo_and_persistent_direction_for_next_recovery() {
    let (mut admin, ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        seata_at::MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    recovery::schema(&mut admin).await;
    let adapter = recovery::adapter(ds);
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    let original = rollback_fixture::snapshot(&mut admin).await;
    sqlx::query("UPDATE undo_work SET v='dirty' WHERE id=1")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        adapter
            .apply(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::ManualInterventionRequired
    );
    let catalog = seata_rm::ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let store = rm_fixture::journal().await;
    let dispatcher = seata_rm::BranchDispatcher::new(catalog, store.clone());
    let identity = receipt.identity();
    assert_eq!(
        dispatcher
            .dispatch(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap(),
        seata_protocol::BranchStatus::PhaseTwoRollbackFailedUnretryable
    );
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(seata_rm::BranchRecordState::Pending)
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 0));
    assert_eq!(rollback_fixture::snapshot(&mut admin).await, original);
    assert!(matches!(
        adapter.apply(receipt.identity(), BranchPhase::Commit).await,
        Err(seata_rm::RmError::IdentityConflict)
    ));
    sqlx::query("UPDATE undo_work SET v='created' WHERE id=1")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        dispatcher
            .dispatch(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap(),
        seata_protocol::BranchStatus::PhaseTwoRollbacked
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(seata_rm::BranchRecordState::Completed)
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn enrollment_is_idempotent_but_does_not_overwrite_application_or_profile() {
    let (mut admin, adapter) = recovery::empty().await;
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    adapter.enroll(&receipt).await.unwrap();
    let changed = recovery::receipt(adapter.clone(), Some("different-application-data")).await;
    assert!(adapter.enroll(&changed).await.is_err());
    assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
    let other = BranchIdentity::new(
        "rollback:xid".into(),
        42,
        adapter.key().clone(),
        Some("different-application-data".into()),
    )
    .unwrap();
    assert!(matches!(
        adapter.apply(&other, BranchPhase::Rollback).await,
        Err(seata_rm::RmError::IdentityConflict)
    ));
    let different = seata_db_mysql::MySqlDataSourceManager::new(
        MySqlDataSource::connect(owned_mysql::options(None))
            .await
            .unwrap(),
        seata_at::MySqlJdbcProfile::Connector8NumericTinyInt,
    )
    .unwrap();
    assert!(different.enroll(&receipt).await.is_err());
    assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn unsupported_commit_binds_direction_without_reporting_success_or_deleting_undo() {
    let (mut admin, adapter) = recovery::empty().await;
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    assert_eq!(
        adapter
            .apply(receipt.identity(), BranchPhase::Commit)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 1, 0));
    assert!(matches!(
        adapter
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await,
        Err(seata_rm::RmError::IdentityConflict)
    ));
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn terminal_write_denial_rolls_back_already_executed_compensation_and_undo_delete() {
    let (mut admin, ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        seata_at::MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    recovery::schema(&mut admin).await;
    let adapter = recovery::adapter(ds);
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET phase=2")
        .execute(&mut admin)
        .await
        .unwrap();
    let original = rollback_fixture::snapshot(&mut admin).await;
    let db = std::env::var("SEATA_MYSQL_DATABASE").unwrap();
    assert!(db.starts_with("undo_") && db.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    for sql in [
        "CREATE USER 'rm_terminal_denied'@'%' IDENTIFIED BY 'seata-mysql-fixture-only'".to_owned(),
        "GRANT PROCESS ON *.* TO 'rm_terminal_denied'@'%'".to_owned(),
        format!("GRANT SELECT,INSERT,DELETE,TRIGGER ON `{db}`.* TO 'rm_terminal_denied'@'%'"),
        format!("GRANT UPDATE ON `{db}`.undo_work TO 'rm_terminal_denied'@'%'"),
    ] {
        sqlx::query(&sql).execute(&mut admin).await.unwrap();
    }
    let options = seata_db_mysql::MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        "rm_terminal_denied".into(),
        "seata-mysql-fixture-only".into(),
        db,
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap();
    let denied = recovery::adapter(MySqlDataSource::connect(options).await.unwrap());
    assert_eq!(
        denied
            .apply(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    assert_eq!(rollback_fixture::snapshot(&mut admin).await, original);
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 0));
    let denied_before = (
        fixture::count(&mut admin, "undo_work").await,
        fixture::count(&mut admin, "undo_log").await,
        recovery::state(&mut admin).await,
    );
    // 补回唯一缺失权限后，同一原身份能继续恢复；不是重新执行业务 SQL。
    sqlx::query(&format!(
        "GRANT UPDATE ON `{}`.seata_rs_at_branch_v1 TO 'rm_terminal_denied'@'%'",
        std::env::var("SEATA_MYSQL_DATABASE").unwrap()
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    assert_eq!(
        denied
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    fixture::save(
        "rm-denial-observation.json",
        &serde_json::json!({"before":denied_before,"after":(fixture::count(&mut admin,"undo_work").await,fixture::count(&mut admin,"undo_log").await,recovery::state(&mut admin).await)}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn recovery_schema_drift_and_triggers_are_rejected_without_enrollment() {
    let (mut admin, adapter) = recovery::empty().await;
    let receipt = recovery::receipt(adapter.clone(), None).await;
    for mutation in ["ALTER TABLE seata_rs_at_branch_v1 MODIFY xid VARCHAR(128) NOT NULL","ALTER TABLE seata_rs_at_branch_v1 ADD COLUMN ignored INT NULL","ALTER TABLE seata_rs_at_branch_v1 ADD CONSTRAINT terminal_check CHECK(terminal=0)","CREATE TRIGGER recovery_trigger BEFORE INSERT ON seata_rs_at_branch_v1 FOR EACH ROW SET NEW.schema_version=1","ALTER TABLE seata_rs_at_branch_v1 DROP PRIMARY KEY","ALTER TABLE seata_rs_at_branch_v1 ADD INDEX extra_index(branch_id)"] {
        recovery::schema(&mut admin).await;sqlx::raw_sql(mutation).execute(&mut admin).await.unwrap();assert!(adapter.enroll(&receipt).await.is_err(),"must reject unsafe recovery metadata: {mutation}");assert_eq!(fixture::count(&mut admin,"seata_rs_at_branch_v1").await,0);
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn concurrent_and_delayed_attempts_observe_one_durable_terminal() {
    let (mut admin, ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        seata_at::MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    recovery::schema(&mut admin).await;
    let adapter = recovery::adapter(ds);
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    let delayed = adapter.apply(receipt.identity(), BranchPhase::Rollback);
    use sqlx::Connection;
    let mut owner = owned_mysql::admin().await;
    let mut held = owner.begin().await.unwrap();
    sqlx::query("SELECT phase FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42 FOR UPDATE")
        .bind(b"rollback:xid".as_slice())
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let adapter = adapter.clone();
        let id = receipt.identity().clone();
        tasks.spawn(async move { adapter.recover(&id, BranchPhase::Rollback).await.unwrap() });
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(1);
    let waits = loop {
        let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='seata_rs_at_branch_v1'").fetch_one(&mut admin).await.unwrap();
        if n >= 2 {
            break n;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "actual competing managed attempts required"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    };
    held.rollback().await.unwrap();
    let mut applied = 0;
    while let Some(value) = tasks.join_next().await {
        if value.unwrap() == BranchResolution::Applied {
            applied += 1;
        }
    }
    assert!(applied > 0);
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    sqlx::query("INSERT INTO undo_work VALUES(1,'late-owner')")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(delayed.await.unwrap(), BranchResolution::Applied);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT v FROM undo_work WHERE id=1")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        "late-owner"
    );
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    fixture::save(
        "rm-concurrent-observation.json",
        &serde_json::json!({"actual_competing_lock_waits":waits,"applied":applied,"terminal":recovery::state(&mut admin).await,"undo":fixture::count(&mut admin,"undo_log").await,"later_business":sqlx::query_scalar::<_,String>("SELECT v FROM undo_work WHERE id=1").fetch_one(&mut admin).await.unwrap()}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture and exact recovery child artifact"]
async fn cancelled_dispatch_retains_pending_and_new_process_recovers_original_identity() {
    use seata_rm::{
        BranchDispatcher, BranchRecordState, FileBranchStore, JournalOptions, ResourceCatalog,
    };
    use sqlx::Connection;
    use std::{path::PathBuf, time::Duration};
    let (mut admin, ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        seata_at::MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    recovery::schema(&mut admin).await;
    let adapter = recovery::adapter(ds);
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    let out = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let journal = out.join("cancelled-dispatch.log");
    let store = FileBranchStore::open(
        journal.clone(),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let dispatcher = BranchDispatcher::new(catalog, store.clone());
    let id = receipt.identity().clone();
    let mut owner = owned_mysql::admin().await;
    let mut lock = owner.begin().await.unwrap();
    sqlx::query(
        "SELECT branch_id FROM undo_log WHERE xid='rollback:xid' AND branch_id=42 FOR UPDATE",
    )
    .fetch_one(&mut *lock)
    .await
    .unwrap();
    let task = tokio::spawn(async move { dispatcher.dispatch(id, BranchPhase::Rollback).await });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let waits = loop {
        let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='undo_log'").fetch_one(&mut admin).await.unwrap();
        if count > 0 {
            break count;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "actual managed Undo lock wait required"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    lock.rollback().await.unwrap();
    let lease = store
        .lease(receipt.identity().clone(), BranchPhase::Rollback)
        .await
        .unwrap();
    assert_eq!(lease.state(), Some(BranchRecordState::Pending));
    drop(lease);
    drop(store);
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 0));
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 1);
    let executable = PathBuf::from(std::env::var("SEATA_RM_CHILD_EXE").unwrap());
    assert!(executable.is_absolute() && executable.to_string_lossy().starts_with("D:"));
    let child_out = out.join("child-recovery.json");
    let result = tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new(executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .env("SEATA_RM_CHILD_JOURNAL", journal)
            .env("SEATA_RM_CHILD_OUT", child_out)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    })
    .await
    .unwrap();
    fixture::save(
        "cancelled-dispatch-child.json",
        &serde_json::json!({"actual_waits":waits,"exit_code":result.status.code(),"stdout":String::from_utf8_lossy(&result.stdout),"stderr":String::from_utf8_lossy(&result.stderr)}),
    );
    assert!(
        result.status.success(),
        "child recovery failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn collation_alias_guard_cannot_publish_another_xid_terminal() {
    let (mut admin, adapter) = recovery::empty().await;
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    sqlx::query("ALTER TABLE undo_log MODIFY xid VARCHAR(128) CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci NOT NULL").execute(&mut admin).await.unwrap();
    for other in ["ROLLBACK:xid", "rôllback:xid"] {
        sqlx::query("DELETE FROM undo_log")
            .execute(&mut admin)
            .await
            .unwrap();
        sqlx::query("INSERT INTO undo_log VALUES(42,?,'serializer=jackson&compressorType=NONE',?,1,now(6),now(6))").bind(other).bind(b"{}".as_slice()).execute(&mut admin).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM undo_log WHERE xid='rollback:xid' AND branch_id=42"
            )
            .fetch_one(&mut admin)
            .await
            .unwrap(),
            1,
            "fixture must actually alias under its MySQL collation"
        );
        assert_eq!(
            adapter
                .apply(receipt.identity(), BranchPhase::Rollback)
                .await
                .unwrap(),
            BranchResolution::Unknown,
            "collation alias guard must not become Applied"
        );
        assert_eq!(recovery::state(&mut admin).await, (1, 2, 0));
        let ds = MySqlDataSource::connect(owned_mysql::options(None))
            .await
            .unwrap();
        assert!(ds.fence_undo_log("rollback:xid", 42).await.is_err());
        ds.close().await.unwrap();
    }
}
