//! 固定原 Java TC 的正式 Rust AT 登记、锁表竞争和回滚释放。
use super::{fixture, owned_mysql, pool, registration};
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_datasource::FieldValue;
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager, MySqlPrepareError};
use seata_protocol::transaction::GlobalLockQueryRequest;
use seata_protocol::{BranchType, GlobalStatus, Message, TransactionExceptionCode};
use seata_rm::{
    BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, ResourceManager, RmCallError,
};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use sqlx::MySqlConnection;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::time::{sleep, Instant};

pub(super) async fn checkpoint(folder: &Path, stage: &str, value: &serde_json::Value) {
    fixture::save(&format!("{stage}.json"), value);
    let ack = folder.join(format!("{stage}.ack"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ack.exists() {
        assert!(
            Instant::now() < deadline,
            "missing independent {stage} observation"
        );
        sleep(Duration::from_millis(10)).await;
    }
}

async fn count_locks(admin: &mut MySqlConnection) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM lock_table")
        .fetch_one(admin)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires pinned original Java TC and owned MySQL lock store"]
async fn native_registration_guard_preserves_exclusive_tc_lock_and_releases_on_original_rollback() {
    let java_owner = std::env::var_os("SEATA_TC_JAVA_LOCK_OWNER").is_some();
    let tc_insert_race = std::env::var_os("SEATA_TC_LOCK_BUNDLE_RACE").is_some();
    #[cfg(feature = "experimental_changed_commit")]
    let rust_owner = std::env::var_os("SEATA_TC_RUST_LOCK_OWNER").is_some();
    #[cfg(not(feature = "experimental_changed_commit"))]
    let rust_owner = false;
    // Java 已持锁时，保留它刚提交的原 Undo。建表由隔离运行器完成。
    let mut admin = if java_owner || rust_owner || tc_insert_race {
        owned_mysql::admin().await
    } else {
        fixture::reset().await
    };
    let writer_user = std::env::var("SEATA_TC_AT_WRITER_USER").ok();
    let recovery_migrations = [
        include_str!("../../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../../migrations/004_seata_rs_at_first_phase_witness.sql"),
    ];
    if writer_user.is_none() {
        for migration in recovery_migrations {
            sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
        }
    }
    sqlx::query("CREATE TABLE guard_text(id VARCHAR(32) PRIMARY KEY) ENGINE=InnoDB COLLATE=utf8mb4_0900_as_cs")
        .execute(&mut admin).await.unwrap();
    assert_eq!(count_locks(&mut admin).await, i64::from(java_owner));
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let combined_profile = std::env::var_os("SEATA_TC_DB_LOCK_PROFILE").is_some();
    let require_binding =
        combined_profile || std::env::var_os("SEATA_TC_REQUIRE_RESOURCE_BINDING").is_some();
    let enforce_lock_limits = std::env::var_os("SEATA_TC_ENFORCE_DB_LOCK_LIMITS").is_some();
    let enforce_collation = std::env::var_os("SEATA_TC_ENFORCE_DB_LOCK_COLLATION").is_some();
    let guard_location = std::env::var("SEATA_TC_LOCK_GUARD_LOCATION").unwrap_or("both".into());
    let bind_tinyint = std::env::var_os("SEATA_TC_BIND_TINYINT_PROFILE").is_some();
    let verify_preprovisioned_tinyint =
        std::env::var_os("SEATA_TC_VERIFY_PREPROVISIONED_TINYINT").is_some();
    let options = |port, apply_lock_checks| {
        let options = if let Some(user) = &writer_user {
            owned_mysql::options_for_user(
                port,
                user.clone(),
                std::env::var("SEATA_TC_AT_WRITER_PASSWORD").unwrap(),
            )
        } else {
            owned_mysql::options(port)
        };
        let options = if combined_profile {
            options.with_java_tc_mysql_db_lock_profile()
        } else {
            options
        };
        let options = if apply_lock_checks && enforce_lock_limits {
            options.with_java_tc_mysql_lock_limits()
        } else {
            options
        };
        let options = if apply_lock_checks && enforce_collation {
            options.with_java_tc_mysql_lock_collation()
        } else {
            options
        };
        if apply_lock_checks && (bind_tinyint || verify_preprovisioned_tinyint) {
            options.require_at_tinyint_profile_binding()
        } else if require_binding {
            options.require_at_resource_binding()
        } else {
            options
        }
    };
    let source = MySqlDataSource::connect(options(None, guard_location != "manager"))
        .await
        .unwrap();
    let manager_source = match guard_location.as_str() {
        "both" => source.clone(),
        "manager" | "transaction" => {
            MySqlDataSource::connect(options(None, guard_location == "manager"))
                .await
                .unwrap()
        }
        _ => panic!("invalid guard location"),
    };
    let profile = if matches!(
        std::env::var("SEATA_TC_ACCOUNT_KEY").as_deref(),
        Ok("tinyint-numeric" | "tinyint-mismatch")
    ) {
        MySqlJdbcProfile::Connector8NumericTinyInt
    } else {
        MySqlJdbcProfile::Connector8Default
    };
    let manager = Arc::new(MySqlDataSourceManager::new(manager_source.clone(), profile).unwrap());
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(manager.clone()).unwrap();
    if require_binding && !verify_preprovisioned_tinyint {
        sqlx::raw_sql(include_str!(
            "../../migrations/005_seata_rs_at_resource_binding.sql"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
            .bind(manager.key().resource_id().as_bytes())
            .execute(&mut admin)
            .await
            .unwrap();
        if bind_tinyint {
            sqlx::raw_sql(include_str!(
                "../../migrations/011_seata_rs_at_tinyint_profile_binding.sql"
            ))
            .execute(&mut admin)
            .await
            .unwrap();
            sqlx::query("UPDATE seata_rs_at_resource_binding SET tinyint1_is_bit=1 WHERE slot=1")
                .execute(&mut admin)
                .await
                .unwrap();
        }
    }
    let alias = if let Ok(port) = std::env::var("SEATA_TC_LOCK_ALIAS_PORT") {
        let port = port.parse().unwrap();
        let source = MySqlDataSource::connect(options(Some(port), true))
            .await
            .unwrap();
        let manager = Arc::new(
            MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default)
                .unwrap(),
        );
        catalog.register(manager.clone()).unwrap();
        Some((port, source, manager))
    } else {
        None
    };
    let journal = FileBranchStore::open(
        folder.join("guard-branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        journal,
    )
    .request_timeout(Duration::from_secs(if tc_insert_race { 20 } else { 2 }))
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
    if tc_insert_race {
        super::lock_bundle_race_cases::exercise(&mut admin, &source, &manager, &rm, &tm, &folder)
            .await;
        tm.shutdown().await.unwrap();
        rm.shutdown().await.unwrap();
        source.close().await.unwrap();
        return;
    }
    #[cfg(feature = "experimental_changed_commit")]
    if rust_owner {
        super::java_rust_lock_cases::rust_holds_lock(
            &mut admin, &source, &manager, &rm, &tm, &folder,
        )
        .await;
        tm.shutdown().await.unwrap();
        rm.shutdown().await.unwrap();
        source.close().await.unwrap();
        return;
    }
    if java_owner {
        if std::env::var_os("SEATA_TC_LOCK_BUNDLE").is_some() {
            super::lock_bundle_cases::exercise(&mut admin, &source, &manager, &rm, &tm, &folder)
                .await;
        } else {
            super::java_rust_lock_cases::exercise(&mut admin, &source, &manager, &rm, &tm, &folder)
                .await;
        }
        tm.shutdown().await.unwrap();
        rm.shutdown().await.unwrap();
        source.close().await.unwrap();
        return;
    }
    if let Some((port, alias_source, alias_manager)) = alias {
        super::lock_identity_cases::exercise(
            &mut admin,
            (&source, &manager),
            (port, &alias_source, &alias_manager),
            &rm,
            &tm,
            &folder,
            (require_binding, combined_profile),
        )
        .await;
        tm.shutdown().await.unwrap();
        rm.shutdown().await.unwrap();
        alias_source.close().await.unwrap();
        source.close().await.unwrap();
        return;
    }
    if std::env::var_os("SEATA_TC_LOCK_COLLATION_EQUIVALENCE").is_some() {
        super::lock_collation_cases::exercise(
            &mut admin,
            &source,
            &manager,
            &rm,
            &tm,
            &folder,
            enforce_collation,
        )
        .await;
        tm.shutdown().await.unwrap();
        rm.shutdown().await.unwrap();
        manager_source.close().await.unwrap();
        source.close().await.unwrap();
        return;
    }
    if std::env::var_os("SEATA_TC_LOCK_STORAGE_BOUNDARIES").is_some() {
        super::lock_storage_cases::exercise(
            &mut admin,
            &source,
            &manager,
            &rm,
            &tm,
            &folder,
            enforce_lock_limits,
        )
        .await;
        tm.shutdown().await.unwrap();
        rm.shutdown().await.unwrap();
        manager_source.close().await.unwrap();
        source.close().await.unwrap();
        return;
    }
    let config = TransactionConfig::builder("native-lock-guard".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut owner = tm.begin(&config).await.unwrap();
    let mut contender = tm.begin(&config).await.unwrap();
    let owner_xid = owner.xid().clone();
    let contender_xid = contender.xid().clone();
    let client = rm.client();
    for key in ["", "a;b"] {
        let mut local = source.begin().await.unwrap();
        assert_eq!(
            local
                .execute(
                    &SqlRecognizerFactory::recognize("INSERT INTO guard_text(id) VALUES(?)")
                        .unwrap(),
                    &[FieldValue::Text(key.into())]
                )
                .await
                .unwrap(),
            1
        );
        assert!(matches!(
            manager
                .prepare_transaction(local, &client, owner_xid.as_str(), None)
                .await,
            Err(MySqlPrepareError::LockKey(_))
        ));
    }
    assert_eq!(count_locks(&mut admin).await, 0);
    let insert =
        SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'exclusive')")
            .unwrap();
    let mut local = source.begin().await.unwrap();
    local.execute(&insert, &[]).await.unwrap();
    let prepared = manager
        .prepare_transaction(local, &client, owner_xid.as_str(), None)
        .await
        .unwrap();
    let receipt = prepared.registered_branch().unwrap().clone();
    let branch_id = receipt.identity().branch_id();
    let resource = manager.key().resource_id();
    assert_eq!(receipt.lock_key(), Some("undo_work:1"));
    let locked: (String, i64, String, String, String, String, i8) = sqlx::query_as(
        "SELECT xid,branch_id,resource_id,table_name,pk,row_key,status FROM lock_table",
    )
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(
        locked,
        (
            owner_xid.as_str().into(),
            branch_id,
            resource.into(),
            "undo_work".into(),
            "1".into(),
            format!("{resource}^^^undo_work^^^1"),
            0
        )
    );
    let base = serde_json::json!({"owner":owner_xid.as_str(),"contender":contender_xid.as_str(),
        "branch_id":branch_id,"resource":resource,"lock_key":"undo_work:1","rejected_locally":2});
    checkpoint(&folder, "held", &base).await;
    prepared
        .lock_commit_preconditions()
        .await
        .unwrap()
        .rollback()
        .await
        .unwrap();
    assert!(!manager.first_phase_commit_observed(&receipt).await.unwrap());
    assert!(
        !client
            .lock_query(GlobalLockQueryRequest {
                xid: Some(contender_xid.as_str().into()),
                branch_type: BranchType::At,
                resource_id: Some(resource.into()),
                lock_key: Some("undo_work:1".into()),
                application_data: None,
            })
            .await
            .unwrap()
            .lockable
    );
    let mut second = source.begin().await.unwrap();
    second.execute(&insert, &[]).await.unwrap();
    let error = manager
        .prepare_transaction(second, &client, contender_xid.as_str(), None)
        .await
        .unwrap_err();
    let MySqlPrepareError::Registration(RmCallError::DefiniteRejected { response, .. }) = error
    else {
        panic!("expected original TC rejection: {error:?}");
    };
    let Some(Message::BranchRegisterResponse(rejected)) = response.body else {
        panic!("invalid original response");
    };
    let exception = rejected.response.transaction_exception_code;
    assert!(matches!(
        exception,
        TransactionExceptionCode::LockKeyConflict
            | TransactionExceptionCode::LockKeyConflictFailFast
    ));
    assert_eq!(count_locks(&mut admin).await, 1);
    checkpoint(&folder, "contended", &base).await;
    assert!(matches!(
        contender.rollback().await.unwrap(),
        TransactionCompletion::Observed { .. }
    ));
    assert!(matches!(
        owner.rollback().await.unwrap(),
        TransactionCompletion::Observed { .. }
    ));
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if tm.get_status(&owner_xid).await.unwrap().status() == GlobalStatus::Finished
            && tm.get_status(&contender_xid).await.unwrap().status() == GlobalStatus::Finished
            && count_locks(&mut admin).await == 0
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "original TC did not release the lock"
        );
        sleep(Duration::from_millis(50)).await;
    }
    let state: (u8, u8, u8) = sqlx::query_as(
        "SELECT phase,terminal,first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?")
        .bind(owner_xid.as_str().as_bytes()).bind(branch_id).fetch_one(&mut admin).await.unwrap();
    assert_eq!(state, (2, 1, 0));
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "guard_text").await, 0);
    let guard: i32 =
        sqlx::query_scalar("SELECT log_status FROM undo_log WHERE xid=? AND branch_id=?")
            .bind(owner_xid.as_str())
            .bind(branch_id)
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!(guard, 1);
    fixture::save(
        "tc-lock-guard-result.json",
        &serde_json::json!({
            "identity":base,"lock_conflict_code":exception.to_java_v1(),"lockable":false,
            "lock_rows_after_rollback":0,"recovery_state":state,"undo_guard":guard,"business_rows":0
        }),
    );
    tm.shutdown().await.unwrap();
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
