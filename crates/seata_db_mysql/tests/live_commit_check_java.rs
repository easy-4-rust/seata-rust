//! 固定原 Java TC + 正式 TM/RM + 实际 MySQL；准备后回滚原本地事务，再观察原 TC 回调。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/java_affected_image_guard.rs"]
mod java_affected_image_guard;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_checksum_events.rs"]
mod java_checksum_events;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_checksum_probe.rs"]
mod java_checksum_probe;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_mysql_control_metrics.rs"]
mod java_mysql_control_metrics;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_mysql_permission.rs"]
mod java_mysql_permission;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_mysql_tls.rs"]
mod java_mysql_tls;
#[path = "support/java_no_change_commit.rs"]
mod java_no_change_commit;
#[path = "support/java_pool_probe.rs"]
mod java_pool_probe;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_report_claim_child.rs"]
mod java_report_claim_child;
#[path = "support/java_rm_call_metrics.rs"]
mod java_rm_call_metrics;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_rm_restart.rs"]
mod java_rm_restart;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_undo_dirty.rs"]
mod java_undo_dirty;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_undo_mixed.rs"]
mod java_undo_mixed;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_undo_orphan.rs"]
mod java_undo_orphan;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/java_witness_query_failure.rs"]
mod java_witness_query_failure;
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
    original_java_tc_missing_undo(false).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_rollback_with_missing_undo_preserves_original_guard() {
    original_java_tc_missing_undo(true).await;
}

async fn original_java_tc_missing_undo(current_schema: bool) {
    let lose_reply =
        current_schema && std::env::var("SEATA_JAVA_AT_CALLBACK_REPLY_LOSS").as_deref() == Ok("1");
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
    let observer_port = if current_schema {
        for migration in [
            include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
            include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        ] {
            sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
        }
        Some(
            std::env::var("SEATA_JAVA_RM_MYSQL_PORT")
                .unwrap()
                .parse()
                .unwrap(),
        )
    } else {
        None
    };
    let ds = MySqlDataSource::connect(owned_mysql::options(observer_port))
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
    let (normal_context, normal_body, normal_status): (String, Vec<u8>, i32) = sqlx::query_as(
        "SELECT context,rollback_info,log_status FROM undo_log WHERE xid=? AND branch_id=?",
    )
    .bind(xid.as_str())
    .bind(original_branch)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(normal_status, 0);
    assert!(!normal_body.is_empty());
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut admin)
        .await
        .unwrap();
    prepared.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    let observed_end = match transaction.rollback().await {
        Ok(TransactionCompletion::Observed { outcome, .. }) => {
            assert!(
                outcome.status() == GlobalStatus::Rollbacked
                    || lose_reply && outcome.status() == GlobalStatus::RollbackRetrying
            );
            Some(outcome.status().to_java_v1())
        }
        Err(seata_tm::TmError::UnknownOutcome {
            operation: seata_tm::TmOperation::Rollback,
            xid: Some(unknown_xid),
            ..
        }) if lose_reply => {
            assert_eq!(unknown_xid, xid);
            None
        }
        other => panic!("{other:?}"),
    };
    if lose_reply {
        // 只查询原 XID。回包丢失时，原 TC 自行重试同一分支。
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            if tm.get_status(&xid).await.unwrap().status() == GlobalStatus::Finished {
                break;
            }
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    // 原成功回执和真实回调的分支号必须一致，不从代理借用分配结果。
    let row:(Vec<u8>,i64,u16,u8,u8,u8,Option<Vec<u8>>)=sqlx::query_as("SELECT xid,branch_id,schema_version,registration_origin,phase,terminal,lock_key FROM seata_rs_at_branch_v1").fetch_one(&mut admin).await.unwrap();
    assert_eq!(row.0, xid.as_str().as_bytes());
    assert_eq!(row.1, original_branch);
    assert_eq!(
        (row.2, row.3, row.4, row.5, row.6),
        (
            if current_schema { 3 } else { 2 },
            1,
            2,
            1,
            Some(b"undo_work:1".to_vec())
        )
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
    if current_schema {
        let witness: u8 =
            sqlx::query_scalar("SELECT first_phase_committed FROM seata_rs_at_branch_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
        assert_eq!(witness, 0);
        let snapshot = "SELECT branch_id,xid,context,HEX(rollback_info),log_status,CAST(log_created AS CHAR),CAST(log_modified AS CHAR) FROM undo_log";
        type GuardRow = (i64, String, String, String, i32, String, String);
        let before: GuardRow = sqlx::query_as(snapshot)
            .fetch_one(&mut admin)
            .await
            .unwrap();
        // 复用此前实际写入但未提交的 Normal Undo，模拟同一分支的迟到写入。
        let error = sqlx::query("INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,?,?,0,NOW(6),NOW(6))")
            .bind(original_branch).bind(xid.as_str()).bind(normal_context).bind(normal_body)
            .execute(&mut admin).await.unwrap_err();
        let duplicate_code = error
            .as_database_error()
            .unwrap()
            .downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .number();
        assert_eq!(duplicate_code, 1062);
        let after: GuardRow = sqlx::query_as(snapshot)
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(fixture::count(&mut admin, "undo_log").await, 1);
        fixture::save(
            "java-missing-undo.json",
            &serde_json::json!({
                "xid":xid.as_str(),"branch_id":row.1,"resource_id":key.resource_id(),
                "observed_end":observed_end,"first_phase_committed":witness,
                "late_normal_error":duplicate_code,"before_late_normal":before,"after_late_normal":after
            }),
        );
    } else {
        fixture::save(
            "java-first-phase-preparation.json",
            &serde_json::json!({"xid":xid.as_str(),"branch_id":row.1,"resource_id":key.resource_id(),"global_result":"Rollbacked","file_result":"Completed","schema_version":2,"origin":1,"phase":2,"terminal":1,"lock_key":"undo_work:1","undo_log_status":1,"business_rows":0,"registration_result":"Success","local_rollback_before_TC_end":true,"uncommitted_Normal_observed":true,"public_business_commit":false,"commit_preconditions_locked":true}),
        );
    }
    if !lose_reply {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if tm.get_status(&xid).await.unwrap().status() == GlobalStatus::Finished {
                break;
            }
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_changed_commit_reports_then_commits_original_branch() {
    original_java_tc_changed_commit(false, None, None).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_recovered_commit_reports_then_commits_original_branch() {
    original_java_tc_changed_commit(false, Some(CommitCase::Confirmed), None).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_lost_success_report_reply_keeps_confirmed_commit_and_receipt() {
    original_java_tc_changed_commit(true, None, None).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw passthrough observer and owned MySQL"]
async fn original_java_tc_retries_rollback_after_missing_undo_root_is_restored() {
    original_java_tc_changed_commit(false, None, Some(RollbackCase::Orphan(false))).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw passthrough observer and owned MySQL"]
async fn original_java_tc_retries_rollback_after_guarded_undo_root_is_restored() {
    original_java_tc_changed_commit(false, None, Some(RollbackCase::Orphan(true))).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, callback reply loss observer and owned MySQL"]
async fn original_java_tc_retries_at_rollback_after_phase_two_fault() {
    original_java_tc_changed_commit(false, None, Some(RollbackCase::ReplyLoss)).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw passthrough observer and owned MySQL"]
async fn original_java_tc_stops_automatic_rollback_retry_for_dirty_undo() {
    original_java_tc_changed_commit(false, None, Some(RollbackCase::Dirty)).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC and owned MySQL statement observer"]
async fn original_java_tc_commits_mixed_compound_key_work() {
    original_java_tc_changed_commit(false, None, Some(RollbackCase::Mixed(BranchPhase::Commit)))
        .await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC and owned MySQL statement observer"]
async fn original_java_tc_rolls_back_mixed_compound_key_work() {
    original_java_tc_changed_commit(
        false,
        None,
        Some(RollbackCase::Mixed(BranchPhase::Rollback)),
    )
    .await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and MySQL report-claim COMMIT response loss"]
async fn original_java_tc_lost_report_claim_commit_reply_never_sends_report() {
    original_java_tc_changed_commit(
        false,
        Some(CommitCase::Confirmed),
        Some(RollbackCase::ReportClaimLoss),
    )
    .await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and supervised report child termination"]
async fn original_java_tc_report_claim_child_exit_never_replays() {
    original_java_tc_changed_commit(
        false,
        Some(CommitCase::Confirmed),
        Some(RollbackCase::ReportChildExit),
    )
    .await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and an owned MySQL held COMMIT acknowledgment"]
async fn original_java_tc_changed_commit_timeout_recycles_both_pool_slots() {
    original_java_tc_changed_commit(false, Some(CommitCase::PoolTimeout), None).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned MySQL TLS and the pinned original Java TC"]
async fn original_java_tc_rejects_wrong_mysql_ca_then_commits_with_trusted_ca() {
    original_java_tc_changed_commit(false, Some(CommitCase::TlsAdmission), None).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned MySQL grants and the pinned original Java TC"]
async fn original_java_tc_retries_commit_after_terminal_column_grant() {
    original_java_tc_changed_commit(false, Some(CommitCase::TerminalPermission), None).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and owned MySQL"]
async fn original_java_tc_rejects_modified_undo_then_recovers_original_bytes() {
    original_java_tc_changed_commit(false, Some(CommitCase::UndoChecksum), None).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned MySQL and pinned Java TC"]
async fn original_java_tc_completes_branch_after_graceful_rm_restart() {
    original_java_tc_changed_commit(false, Some(CommitCase::GracefulRestart), None).await;
}

#[cfg(feature = "experimental_changed_commit")]
enum CommitCase {
    Confirmed,
    PoolTimeout,
    TlsAdmission,
    TerminalPermission,
    UndoChecksum,
    GracefulRestart,
}

#[cfg(feature = "experimental_changed_commit")]
enum RollbackCase {
    ReportClaimLoss,
    ReportChildExit,
    Dirty,
    Orphan(bool),
    ReplyLoss,
    Mixed(BranchPhase),
}

#[cfg(feature = "experimental_changed_commit")]
async fn original_java_tc_changed_commit(
    lose_reply: bool,
    recovery: Option<CommitCase>,
    rollback_case: Option<RollbackCase>,
) {
    use sqlx::Row;

    // 保留原 Java TC 的 15 秒回调期限。丢确认用例的 TM 请求和终态观察都跨越该期限。
    let callback_reply_loss =
        std::env::var("SEATA_JAVA_AT_CALLBACK_REPLY_LOSS").as_deref() == Ok("1");
    let mut admin = fixture::reset().await;
    let recover_before_report = matches!(
        recovery,
        Some(CommitCase::Confirmed | CommitCase::PoolTimeout | CommitCase::UndoChecksum)
    );
    let tls_admission = matches!(recovery, Some(CommitCase::TlsAdmission));
    let shutdown_callback = std::env::var("SEATA_CALLBACK_SHUTDOWN").as_deref() == Ok("1");
    let restart_rm = matches!(recovery, Some(CommitCase::GracefulRestart));
    let checksum = matches!(recovery, Some(CommitCase::UndoChecksum));
    let terminal_permission = matches!(recovery, Some(CommitCase::TerminalPermission));
    let recycle_pool = matches!(recovery, Some(CommitCase::PoolTimeout));
    let mixed = matches!(rollback_case, Some(RollbackCase::Mixed(_)));
    if mixed {
        java_undo_mixed::seed_table(&mut admin).await;
    }
    for migration in [
        include_str!("../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/005_seata_rs_at_resource_binding.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    if checksum || restart_rm {
        sqlx::raw_sql(include_str!(
            "../migrations/013_seata_rs_at_undo_checksum.sql"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
    }
    let rm_mysql_port = std::env::var("SEATA_JAVA_RM_MYSQL_PORT")
        .ok()
        .map(|port| port.parse().unwrap());
    let tm = TransactionManager::builder(
        pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
        registration(),
    )
    .request_timeout(Duration::from_secs(if callback_reply_loss {
        25
    } else {
        10
    }))
    .build()
    .unwrap();
    let cfg = TransactionConfig::builder("mysql-changed-report-original-tc".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let (options, early_global) = if tls_admission {
        assert!(
            rm_mysql_port.is_none(),
            "TLS requires a direct connection to MySQL"
        );
        let global = tm.begin(&cfg).await.unwrap();
        (
            java_mysql_tls::admit(&folder, global.xid().as_str()).await,
            Some(global),
        )
    } else if terminal_permission {
        (
            java_mysql_permission::restricted_options(&mut admin, rm_mysql_port).await,
            None,
        )
    } else {
        (owned_mysql::options(rm_mysql_port), None)
    };
    let options = options.with_java_tc_mysql_db_lock_profile();
    let options = if recycle_pool {
        options.pool_limits(2, Duration::from_secs(3)).unwrap()
    } else {
        options
    };
    let ds = MySqlDataSource::connect(options).await.unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(ds.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let key = adapter.key().clone();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(key.resource_id().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let store = FileBranchStore::open(
        folder.join("java-changed-report-branches.log"),
        JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let mut rm = ResourceManager::builder(
        pool(&std::env::var("SEATA_RM_ENDPOINT").unwrap()),
        registration(),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(3))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let mut global = match early_global {
        Some(global) => global,
        None => tm.begin(&cfg).await.unwrap(),
    };
    let xid = global.xid().clone();
    let mut local = ds.begin().await.unwrap();
    if mixed {
        java_undo_mixed::execute_mixed(&mut local).await;
    } else {
        local
            .execute(
                &SqlRecognizerFactory::recognize(
                    "INSERT INTO undo_work(id,v) VALUES(1,'original_tc_report')",
                )
                .unwrap(),
                &[],
            )
            .await
            .unwrap();
    }
    let checked = adapter
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    let branch_id = checked.registered_branch().unwrap().identity().branch_id();
    let report_claim_loss = matches!(rollback_case, Some(RollbackCase::ReportClaimLoss));
    let report_child_exit = matches!(rollback_case, Some(RollbackCase::ReportChildExit));
    if recover_before_report {
        if recycle_pool {
            use seata_db_mysql::{DbError, MySqlChangedCommitError};

            let original = checked.registered_branch().unwrap().clone();
            let error = checked.commit_changed(&adapter).await.expect_err(
                "held original changed COMMIT OK must leave the business result unknown",
            );
            assert!(error.is_unknown_outcome());
            let MySqlChangedCommitError::Unknown {
                receipt,
                source:
                    DbError::Deadline {
                        operation: "changed COMMIT",
                    },
            } = error
            else {
                panic!("expected changed COMMIT deadline: {error:?}");
            };
            assert_eq!(receipt.identity(), original.identity());
            assert_eq!(receipt.response(), original.response());
            assert_eq!(receipt.lock_key(), original.lock_key());
            fixture::save(
                "java-changed-pool-local.json",
                &serde_json::json!({
                    "xid": xid.as_str(), "branch_id": branch_id, "resource_id": key.resource_id(),
                    "local_outcome": "Unknown", "source": "Deadline",
                }),
            );
            java_pool_probe::exercise(&ds, &folder, xid.as_str(), 2).await;
            java_pool_probe::wait_for_snapshot(&folder, "java-changed-pool-recovery-permitted")
                .await;
        } else {
            let receipt = checked.commit_changed(&adapter).await.unwrap();
            assert_eq!(receipt.identity().branch_id(), branch_id);
        }
        if std::env::var("SEATA_CHECKSUM_LEGACY").as_deref() == Ok("1") {
            sqlx::raw_sql(include_str!(
                "../migrations/013_seata_rs_at_undo_checksum.sql"
            ))
            .execute(&mut admin)
            .await
            .unwrap();
            let old: (u16, Option<Vec<u8>>) = sqlx::query_as("SELECT schema_version,undo_sha256 FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?")
                .bind(xid.as_str().as_bytes()).bind(branch_id).fetch_one(&mut admin).await.unwrap();
            assert_eq!(old, (3, None));
            fixture::save(
                "java-checksum-legacy.json",
                &serde_json::json!({
                    "xid":xid.as_str(),"branch_id":branch_id,"version":old.0,"digest_absent":true
                }),
            );
        }
        if checksum {
            java_checksum_probe::reject_modified_undo(
                &adapter,
                &rm.client(),
                &mut admin,
                xid.as_str(),
                branch_id,
            )
            .await;
        }
        assert_eq!(
            adapter
                .recover_commit_attempt(xid.as_str(), branch_id)
                .await
                .unwrap(),
            seata_db_mysql::MySqlCommitRecoveryOutcome::Applied
        );
        if report_child_exit {
            java_report_claim_child::wait_for_exit(&folder, xid.as_str(), branch_id).await;
        } else {
            let report = adapter
                .report_recovered_applied(&rm.client(), xid.as_str(), branch_id)
                .await;
            if report_claim_loss {
                assert!(matches!(
                    report,
                    Err(seata_db_mysql::MySqlRecoveredReportError::Claim { .. })
                ));
            } else {
                report.unwrap();
            }
        }
        if recycle_pool || report_claim_loss || report_child_exit {
            let started: u8 = sqlx::query_scalar(
                "SELECT report_started FROM seata_rs_at_commit_attempt WHERE xid=? AND branch_id=?",
            )
            .bind(xid.as_str().as_bytes())
            .bind(branch_id)
            .fetch_one(&mut admin)
            .await
            .unwrap();
            assert_eq!(started, 1);
            assert!(matches!(
                adapter
                    .report_recovered_applied(&rm.client(), xid.as_str(), branch_id)
                    .await,
                Err(seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted)
            ));
        }
    } else if lose_reply {
        let error = checked
            .commit_changed_and_report(&adapter, &rm.client())
            .await
            .unwrap_err();
        assert!(error.local_commit_confirmed());
        assert!(error.is_unknown_outcome());
        assert_eq!(
            error.registered_branch().unwrap().identity().branch_id(),
            branch_id
        );
    } else {
        let receipt = checked
            .commit_changed_and_report(&adapter, &rm.client())
            .await
            .unwrap();
        assert_eq!(receipt.identity().branch_id(), branch_id);
    }
    let before: (i64, i64, u8, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM undo_work), (SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0), (SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?), (SELECT COUNT(*) FROM lock_table WHERE xid=? AND branch_id=?)",
    )
    .bind(xid.as_str())
    .bind(branch_id)
    .bind(xid.as_str().as_bytes())
    .bind(branch_id)
    .bind(xid.as_str())
    .bind(branch_id)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(before, if mixed { (4, 1, 1, 2) } else { (1, 1, 1, 1) });
    let attempt: (Vec<u8>, i64, Vec<u8>, Option<Vec<u8>>, u8) = sqlx::query_as(
        "SELECT xid,branch_id,resource_id,lock_key,attempt_state FROM seata_rs_at_commit_attempt",
    )
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(attempt.0, xid.as_str().as_bytes());
    assert_eq!(attempt.1, branch_id);
    assert_eq!(attempt.2, key.resource_id().as_bytes());
    let lock_key = String::from_utf8(attempt.3.unwrap()).unwrap();
    if !mixed {
        assert_eq!(lock_key, "undo_work:1");
        let pk: String =
            sqlx::query_scalar("SELECT pk FROM lock_table WHERE xid=? AND branch_id=?")
                .bind(xid.as_str())
                .bind(branch_id)
                .fetch_one(&mut admin)
                .await
                .unwrap();
        assert_eq!(pk, "1");
    }
    assert_eq!(attempt.4, 1);
    if let Some(rollback_case) = rollback_case {
        let identity = BranchIdentity::new(xid.as_str().into(), branch_id, key, None).unwrap();
        match rollback_case {
            RollbackCase::Mixed(phase) => {
                java_undo_mixed::exercise(
                    &mut admin,
                    &mut global,
                    &tm,
                    &store,
                    &identity,
                    phase,
                    &lock_key,
                )
                .await;
            }
            RollbackCase::Dirty => {
                java_undo_dirty::exercise(&mut admin, &mut global, &store, &identity).await;
            }
            RollbackCase::Orphan(existing_guard) => {
                java_undo_orphan::exercise(
                    &mut admin,
                    &mut global,
                    &tm,
                    &store,
                    &identity,
                    existing_guard,
                )
                .await;
            }
            RollbackCase::ReplyLoss
            | RollbackCase::ReportClaimLoss
            | RollbackCase::ReportChildExit => {
                let observed_end = match global.rollback().await {
                    Ok(TransactionCompletion::Observed { outcome, .. }) => {
                        assert!(matches!(
                            outcome.status(),
                            GlobalStatus::RollbackRetrying | GlobalStatus::Rollbacked
                        ));
                        Some(outcome.status().to_java_v1())
                    }
                    Err(seata_tm::TmError::UnknownOutcome {
                        operation: seata_tm::TmOperation::Rollback,
                        xid: Some(unknown_xid),
                        ..
                    }) => {
                        assert_eq!(unknown_xid, xid);
                        None
                    }
                    other => panic!("unexpected original rollback result: {other:?}"),
                };
                // 即使 TM 失去响应，也只查询原 XID。由 TC 发起二阶段重试。
                let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
                loop {
                    if tm.get_status(&xid).await.unwrap().status() == GlobalStatus::Finished {
                        break;
                    }
                    assert!(tokio::time::Instant::now() < deadline);
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                assert_eq!(
                    store
                        .lease(identity.clone(), BranchPhase::Rollback)
                        .await
                        .unwrap()
                        .state(),
                    Some(BranchRecordState::Completed)
                );
                fixture::save(
                    if report_claim_loss {
                        "java-report-claim-loss.json"
                    } else if report_child_exit {
                        "java-report-claim-child-exit.json"
                    } else {
                        "java-rollback-callback-loss.json"
                    },
                    &serde_json::json!({
                        "xid": identity.xid(), "branch_id": identity.branch_id(),
                        "resource_id": identity.resource().resource_id(), "observed_end": observed_end,
                        "report_claim_unknown_with_started_marker": report_claim_loss,
                        "report_child_exited_without_sending": report_child_exit
                    }),
                );
            }
        }
        if std::env::var("SEATA_MYSQL_CONTROL_METRICS").as_deref() == Ok("1") {
            java_mysql_control_metrics::save(&ds.recovery_metrics_sampler());
        }
        rm.shutdown().await.unwrap();
        tm.shutdown().await.unwrap();
        ds.close().await.unwrap();
        return;
    }
    if restart_rm && !shutdown_callback {
        rm = java_rm_restart::restart(rm, &adapter, &store, &mut admin, &xid, branch_id).await;
    }
    if checksum {
        let original: Vec<u8> =
            sqlx::query_scalar("SELECT rollback_info FROM undo_log WHERE xid=? AND branch_id=?")
                .bind(xid.as_str())
                .bind(branch_id)
                .fetch_one(&mut admin)
                .await
                .unwrap();
        sqlx::query("UPDATE undo_log SET rollback_info=REPLACE(rollback_info,_binary'original_tc_report',_binary'modified_tc_report') WHERE xid=? AND branch_id=?")
            .bind(xid.as_str()).bind(branch_id).execute(&mut admin).await.unwrap();
        fixture::save(
            "java-checksum-callback.json",
            &serde_json::json!({"xid":xid.as_str(),"branch_id":branch_id,"original":original}),
        );
    }
    if checksum && std::env::var("SEATA_CHECKSUM_ROLLBACK").as_deref() == Ok("1") {
        let observed_end = match global.rollback().await.unwrap() {
            TransactionCompletion::Observed { outcome, .. } => outcome.status().to_java_v1(),
            other => panic!("{other:?}"),
        };
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        loop {
            if tm.get_status(&xid).await.unwrap().status() == GlobalStatus::Finished {
                break;
            }
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
        fixture::save(
            "java-checksum-rollback.json",
            &serde_json::json!({
                "xid":xid.as_str(),"branch_id":branch_id,"resource_id":key.resource_id(),
                "observed_end":observed_end
            }),
        );
        if std::env::var("SEATA_MYSQL_CONTROL_METRICS").as_deref() == Ok("1") {
            java_mysql_control_metrics::save(&ds.recovery_metrics_sampler());
        }
        rm.shutdown().await.unwrap();
        tm.shutdown().await.unwrap();
        ds.close().await.unwrap();
        return;
    }
    if terminal_permission {
        java_mysql_permission::wait_for_baseline(&folder, xid.as_str(), branch_id).await;
    }
    if shutdown_callback && std::env::var("SEATA_CALLBACK_ROLLBACK_SHUTDOWN").as_deref() == Ok("1")
    {
        let observed_end = {
            let call = global.rollback();
            tokio::pin!(call);
            tokio::select! {
                value = &mut call => panic!("original rollback completed before held callback shutdown: {value:?}"),
                _ = java_pool_probe::wait_for_snapshot(&folder, "java-callback-shutdown-permitted") => {}
            }
            rm = java_rm_restart::restart(rm, &adapter, &store, &mut admin, &xid, branch_id).await;
            match call.await {
                Ok(TransactionCompletion::Observed { outcome, .. }) => {
                    Some(outcome.status().to_java_v1())
                }
                Err(error) if error.is_unknown_outcome() => None,
                other => panic!("{other:?}"),
            }
        };
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
        let identity =
            BranchIdentity::new(xid.as_str().into(), branch_id, key.clone(), None).unwrap();
        assert_eq!(
            store
                .lease(identity, BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        fixture::save(
            "java-callback-shutdown-rollback.json",
            &serde_json::json!({
                "xid":xid.as_str(),"branch_id":branch_id,"resource_id":key.resource_id(),"observed_end":observed_end
            }),
        );
        fixture::save(
            "rm-branch-metrics-final.json",
            &java_rm_restart::value(
                rm.metrics_sampler()
                    .snapshot(seata_rm::BranchPhase::Rollback),
            ),
        );
        if std::env::var("SEATA_MYSQL_CONTROL_METRICS").as_deref() == Ok("1") {
            java_mysql_control_metrics::save(&ds.recovery_metrics_sampler());
        }
        rm.shutdown().await.unwrap();
        tm.shutdown().await.unwrap();
        ds.close().await.unwrap();
        return;
    }
    match global.commit().await.unwrap() {
        TransactionCompletion::Observed { outcome, .. } => {
            assert_eq!(outcome.status(), GlobalStatus::Committed)
        }
        other => panic!("{other:?}"),
    }
    if shutdown_callback {
        java_pool_probe::wait_for_snapshot(&folder, "java-callback-shutdown-permitted").await;
        rm = java_rm_restart::restart(rm, &adapter, &store, &mut admin, &xid, branch_id).await;
    }
    let deadline = tokio::time::Instant::now()
        + Duration::from_secs(if callback_reply_loss || shutdown_callback {
            30
        } else {
            15
        });
    loop {
        let row = sqlx::query("SELECT first_phase_committed,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?")
            .bind(xid.as_str().as_bytes())
            .bind(branch_id)
            .fetch_one(&mut admin)
            .await
            .unwrap();
        if (
            row.get::<u8, _>("first_phase_committed"),
            row.get::<u8, _>("commit_authorized"),
            row.get::<u8, _>("commit_terminal"),
        ) == (1, 1, 1)
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC commit callback required"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    loop {
        if tm.get_status(&xid).await.unwrap().status() == GlobalStatus::Finished {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC final cleanup required"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // MySQL 的终态可先于丢确认后的 RM 重试可见，等 TC 收到成功回调再读取日志终态。
    let identity = BranchIdentity::new(xid.as_str().into(), branch_id, key.clone(), None).unwrap();
    assert_eq!(
        store
            .lease(identity, BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    fixture::save(
        if recover_before_report {
            "java-changed-report-recovered.json"
        } else if lose_reply {
            "java-changed-report-loss.json"
        } else {
            "java-changed-report.json"
        },
        &serde_json::json!({"xid":xid.as_str(),"branch_id":branch_id,"resource_id":key.resource_id(),"before_commit":{"business":before.0,"normal_undo":before.1,"witness":before.2,"tc_lock":before.3},"attempt_rows":1,"report_response":if lose_reply {"Lost"} else {"Success"},"recovery_outcome":if recover_before_report {Some("Applied")} else {None},"local_commit_confirmed":true,"original_receipt_preserved":true,"global_result":"Committed","callback":"Completed","business_rows":1,"undo_rows":0,"first_phase_committed":1}),
    );
    if std::env::var("SEATA_CALLBACK_SHUTDOWN").as_deref() == Ok("1") {
        fixture::save(
            "rm-branch-metrics-final.json",
            &java_rm_restart::value(rm.metrics_sampler().snapshot(seata_rm::BranchPhase::Commit)),
        );
    }
    if std::env::var("SEATA_RM_CALL_METRICS").as_deref() == Ok("1") {
        java_rm_call_metrics::save(&rm.call_metrics_sampler(), "rm-call-metrics.json");
    }
    if std::env::var("SEATA_MYSQL_CONTROL_METRICS").as_deref() == Ok("1") {
        java_mysql_control_metrics::save(&ds.recovery_metrics_sampler());
    }
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "child process controlled by original_java_tc_cross_process_recovery_reports_original_branch"]
async fn original_java_tc_cross_process_commit_child() {
    if std::env::var("SEATA_JAVA_TC_COMMIT_CHILD").as_deref() != Ok("1") {
        return;
    }
    let xid = std::env::var("SEATA_JAVA_TC_COMMIT_XID").unwrap();
    let upstream = std::env::var("SEATA_JAVA_RM_MYSQL_PORT")
        .ok()
        .map(|port| port.parse().unwrap());
    let ds = MySqlDataSource::connect(
        owned_mysql::options(upstream).with_java_tc_mysql_db_lock_profile(),
    )
    .await
    .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(ds.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let store = FileBranchStore::open(
        folder.join("java-cross-process-child-branches.log"),
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
        store,
    )
    .request_timeout(Duration::from_secs(3))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let mut local = ds.begin().await.unwrap();
    local
        .execute(
            &SqlRecognizerFactory::recognize(
                "INSERT INTO undo_work(id,v) VALUES(1,'original_tc_cross_process')",
            )
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
    let checked = adapter
        .prepare_transaction(local, &rm.client(), &xid, None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    if std::env::var("SEATA_JAVA_TC_ARM_ACK_LOSS").as_deref() == Ok("1") {
        use seata_db_mysql::{MySqlChangedCommitError, MySqlRecoveryError};

        let error = checked
            .commit_changed(&adapter)
            .await
            .expect_err("missing 006 COMMIT OK must stop the original business COMMIT");
        assert!(error.is_unknown_outcome());
        let MySqlChangedCommitError::Arming {
            receipt,
            source: MySqlRecoveryError::CommitOutcomeUnknown { identity, .. },
            rollback: Ok(()),
        } = error
        else {
            panic!("expected unknown arming and a confirmed business rollback: {error:?}");
        };
        assert_eq!(receipt.identity(), identity.as_ref());
        assert_eq!(receipt.identity().xid(), xid);
        assert_eq!(receipt.lock_key(), Some("undo_work:1"));
        fixture::save(
            "java-arming-ack-loss-child.json",
            &serde_json::json!({
                "xid": xid, "branch_id": receipt.identity().branch_id(),
                "resource_id": receipt.identity().resource().resource_id(),
                "child_pid": std::process::id(), "arming": "CommitOutcomeUnknown",
                "business_rollback": "Confirmed",
            }),
        );
    } else {
        let receipt = checked.commit_changed(&adapter).await.unwrap();
        assert_eq!(receipt.identity().xid(), xid);
    }
    // 故意退出而不执行 RM shutdown：新进程只能从原数据库记录恢复身份。
    std::process::exit(0);
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "child process controlled by original_java_tc_distinct_processes_compete_for_report"]
async fn original_java_tc_report_race_child() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    if std::env::var("SEATA_JAVA_TC_REPORT_RACE_CHILD").as_deref() != Ok("1") {
        return;
    }
    let xid = std::env::var("SEATA_JAVA_TC_COMMIT_XID").unwrap();
    let branch_id = std::env::var("SEATA_JAVA_TC_COMMIT_BRANCH")
        .unwrap()
        .parse()
        .unwrap();
    let ds =
        MySqlDataSource::connect(owned_mysql::options(None).with_java_tc_mysql_db_lock_profile())
            .await
            .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(ds.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let store = FileBranchStore::open(
        folder.join(format!("java-report-race-{}.log", std::process::id())),
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
        store,
    )
    .request_timeout(Duration::from_secs(5))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let mut barrier =
        TcpStream::connect(std::env::var("SEATA_JAVA_TC_REPORT_RACE_BARRIER").unwrap())
            .await
            .unwrap();
    let mut signal = [0_u8; 1];
    tokio::time::timeout(Duration::from_secs(10), barrier.read_exact(&mut signal))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(signal[0], 1);
    let result = adapter
        .report_recovered_applied(&rm.client(), &xid, branch_id)
        .await;
    let outcome = match result {
        Ok(()) => 1,
        Err(seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted) => 2,
        other => panic!("unexpected cross-process report outcome: {other:?}"),
    };
    barrier.write_all(&[outcome]).await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), barrier.read_exact(&mut signal))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(signal[0], 2);
    rm.shutdown().await.unwrap();
    ds.close().await.unwrap();
    println!("REPORT_RACE={outcome}");
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_cross_process_recovery_reports_original_branch() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::Commit).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_cross_process_concurrent_recovery_reports_once() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::ConcurrentCommit).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC and two owned recovery processes"]
async fn original_java_tc_distinct_processes_compete_for_report() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::DistinctCommit).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and a supervised child with its business COMMIT held"]
async fn original_java_tc_armed_child_crash_before_business_commit() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::BlockedBusinessCommit).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and a supervised child with its business COMMIT OK held"]
async fn original_java_tc_armed_child_crash_after_business_commit_before_ack() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::BusinessCommitAckHeld).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and a supervised child with its business COMMIT executing"]
async fn original_java_tc_armed_child_crash_during_business_commit() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::BusinessCommitAckHeld).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC, a supervised business child and a lost fence COMMIT OK"]
async fn original_java_tc_armed_child_fence_ack_loss_remains_unknown_until_reobserved() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::FenceAckLost).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and a real MySQL lock timeout on the original 004 witness"]
async fn original_java_tc_armed_child_witness_query_failure_remains_unknown() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::WitnessQueryFailure).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and a lost original 006 COMMIT OK"]
async fn original_java_tc_arming_ack_loss_suppresses_business_commit() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::ArmingAckLost).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and a 006 COMMIT blocked before reaching MySQL"]
async fn original_java_tc_arming_commit_blocked_leaves_attempt_missing() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::ArmingCommitBlocked).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and a synchronized old MySQL owner rollback"]
async fn original_java_tc_recovery_waits_for_old_business_rollback() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::WaitForBusinessRollback).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned Java TC and a synchronized old MySQL owner commit"]
async fn original_java_tc_recovery_waits_for_old_business_commit() {
    original_java_tc_cross_process_recovery(CrossProcessRecovery::WaitForBusinessCommit).await;
}

#[cfg(feature = "experimental_changed_commit")]
enum CrossProcessRecovery {
    Commit,
    ConcurrentCommit,
    DistinctCommit,
    BlockedBusinessCommit,
    BusinessCommitAckHeld,
    FenceAckLost,
    WitnessQueryFailure,
    ArmingAckLost,
    ArmingCommitBlocked,
    WaitForBusinessCommit,
    WaitForBusinessRollback,
}

#[cfg(feature = "experimental_changed_commit")]
async fn wait_for_business_child_marker(folder: &std::path::Path, marker: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(25);
    while !folder.join(marker).is_file() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "missing business child marker {marker}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(feature = "experimental_changed_commit")]
async fn original_java_tc_cross_process_recovery(case: CrossProcessRecovery) {
    use sqlx::Row;
    use std::process::{Command, Stdio};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    let concurrent = matches!(
        case,
        CrossProcessRecovery::ConcurrentCommit | CrossProcessRecovery::DistinctCommit
    );
    let distinct_processes = matches!(case, CrossProcessRecovery::DistinctCommit);
    let fence_ack_lost = matches!(case, CrossProcessRecovery::FenceAckLost);
    let witness_query_failure = matches!(case, CrossProcessRecovery::WitnessQueryFailure);
    let arming_blocked = matches!(case, CrossProcessRecovery::ArmingCommitBlocked);
    let wait_rollback = matches!(case, CrossProcessRecovery::WaitForBusinessRollback);
    let wait_commit = matches!(case, CrossProcessRecovery::WaitForBusinessCommit);
    let competing = wait_commit || wait_rollback;
    let blocked = arming_blocked
        || wait_rollback
        || matches!(case, CrossProcessRecovery::BlockedBusinessCommit)
        || fence_ack_lost
        || witness_query_failure
        || matches!(case, CrossProcessRecovery::ArmingAckLost);
    let ack_held = wait_commit || matches!(case, CrossProcessRecovery::BusinessCommitAckHeld);
    let mut admin = fixture::reset().await;
    for migration in [
        include_str!("../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/005_seata_rs_at_resource_binding.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    let upstream = std::env::var("SEATA_JAVA_RM_MYSQL_PORT")
        .ok()
        .map(|port| port.parse().unwrap());
    let options = owned_mysql::options(upstream).with_java_tc_mysql_db_lock_profile();
    let options = if witness_query_failure {
        options.operation_timeout(Duration::from_secs(8)).unwrap()
    } else {
        options
    };
    let ds = MySqlDataSource::connect(options).await.unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(ds.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let key = adapter.key().clone();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(key.resource_id().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let tm = TransactionManager::builder(
        pool(&std::env::var("SEATA_TM_ENDPOINT").unwrap()),
        registration(),
    )
    .request_timeout(Duration::from_secs(10))
    .build()
    .unwrap();
    let cfg = TransactionConfig::builder("mysql-cross-process-original-tc".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut global = tm.begin(&cfg).await.unwrap();
    let xid = global.xid().clone();
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    if blocked || ack_held {
        fixture::save(
            "java-armed-child-ready.json",
            &serde_json::json!({
                "xid": xid.as_str(), "parent_pid": std::process::id()
            }),
        );
        let marker = if competing {
            "java-armed-child-compete"
        } else {
            "java-armed-child-exited"
        };
        wait_for_business_child_marker(&folder, marker).await;
    } else {
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("original_java_tc_cross_process_commit_child")
            .arg("--exact")
            .arg("--ignored")
            .arg("--test-threads=1")
            .env("SEATA_JAVA_TC_COMMIT_CHILD", "1")
            .env("SEATA_JAVA_TC_COMMIT_XID", xid.as_str())
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "child failed: stdout={} stderr={}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }
    let identity_sql = if arming_blocked {
        "SELECT branch_id,resource_id FROM seata_rs_at_branch_v1 WHERE xid=?"
    } else {
        "SELECT branch_id,resource_id FROM seata_rs_at_commit_attempt WHERE xid=?"
    };
    let (branch_id, resource_id): (i64, Vec<u8>) = sqlx::query_as(identity_sql)
        .bind(xid.as_str().as_bytes())
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(resource_id, key.resource_id().as_bytes());
    let attempt = adapter
        .load_commit_attempt(xid.as_str(), branch_id)
        .await
        .unwrap();
    if arming_blocked {
        assert!(
            attempt.is_none(),
            "blocked 006 COMMIT must leave no durable attempt"
        );
    } else {
        let attempt = attempt.unwrap();
        assert_eq!(attempt.identity().branch_id(), branch_id);
        assert_eq!(attempt.lock_key(), Some("undo_work:1"));
    }
    if witness_query_failure {
        java_witness_query_failure::exercise(&adapter, xid.as_str(), branch_id, &folder).await;
    }
    if fence_ack_lost {
        use seata_db_mysql::{MySqlCommitRecoveryError, UndoFenceError};

        let error = adapter
            .recover_commit_attempt(xid.as_str(), branch_id)
            .await
            .expect_err("a lost fence COMMIT reply must remain unknown");
        assert!(error.is_unknown_outcome());
        match error {
            MySqlCommitRecoveryError::Fence {
                source:
                    UndoFenceError::CommitOutcomeUnknown {
                        xid: fence_xid,
                        branch_id: fence_branch_id,
                        ..
                    },
            } => {
                assert_eq!(fence_xid, xid.as_str());
                assert_eq!(fence_branch_id, branch_id);
            }
            other => panic!("expected the fence COMMIT to remain unknown: {other:?}"),
        }
        fixture::save(
            "java-fence-ack-unknown.json",
            &serde_json::json!({
                "xid": xid.as_str(), "branch_id": branch_id, "first_recovery": "FenceCommitOutcomeUnknown"
            }),
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !folder.join("java-fence-ack-reobserve").is_file() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "independent fence snapshot missing"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    let recovered = adapter
        .recover_commit_attempt(xid.as_str(), branch_id)
        .await;
    if arming_blocked {
        let error = recovered.unwrap_err();
        assert!(error.is_unknown_outcome());
        assert!(matches!(
            error,
            seata_db_mysql::MySqlCommitRecoveryError::MissingAttempt
        ));
    } else {
        let expected = if blocked {
            seata_db_mysql::MySqlCommitRecoveryOutcome::NotAppliedFenced
        } else {
            seata_db_mysql::MySqlCommitRecoveryOutcome::Applied
        };
        assert_eq!(recovered.unwrap(), expected);
    }
    if competing {
        fixture::save(
            "java-armed-child-race-result.json",
            &serde_json::json!({
                "xid": xid.as_str(), "branch_id": branch_id,
                "first_recovery": if wait_commit { "Applied" } else { "NotAppliedFenced" },
            }),
        );
        wait_for_business_child_marker(&folder, "java-armed-child-exited").await;
    }
    let store = FileBranchStore::open(
        folder.join("java-cross-process-parent-branches.log"),
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
    .request_timeout(Duration::from_secs(3))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    if blocked {
        use seata_db_mysql::MySqlRecoveredReportError;

        let report = adapter
            .report_recovered_applied(&rm.client(), xid.as_str(), branch_id)
            .await;
        if arming_blocked {
            assert!(matches!(
                report,
                Err(MySqlRecoveredReportError::Recovery(
                    seata_db_mysql::MySqlCommitRecoveryError::MissingAttempt
                ))
            ));
        } else {
            assert!(matches!(
                report,
                Err(MySqlRecoveredReportError::NotAppliedFenced)
            ));
        }
        let before: (i64, i64, u8, Option<u8>) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM undo_work),(SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=1), \
             b.first_phase_committed,a.report_started FROM seata_rs_at_branch_v1 b \
             LEFT JOIN seata_rs_at_commit_attempt a USING(xid,branch_id) WHERE b.xid=? AND b.branch_id=?",
        ).bind(xid.as_str()).bind(branch_id).bind(xid.as_str().as_bytes()).bind(branch_id)
            .fetch_one(&mut admin).await.unwrap();
        assert_eq!(
            before,
            if arming_blocked {
                (0, 0, 0, None)
            } else {
                (0, 1, 0, Some(0))
            }
        );
        let deadline = tokio::time::Instant::now() + Duration::from_secs(12);
        while rm.connection_snapshot().ready() == 0 {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let observed_end = match global.rollback().await.unwrap() {
            TransactionCompletion::Observed { outcome, .. } => {
                assert_eq!(outcome.status(), GlobalStatus::Rollbacked);
                outcome.status().to_java_v1()
            }
            other => panic!("original blocked-commit rollback failed: {other:?}"),
        };
        while tm.get_status(&xid).await.unwrap().status() != GlobalStatus::Finished {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let identity =
            BranchIdentity::new(xid.as_str().into(), branch_id, key.clone(), None).unwrap();
        assert_eq!(
            store
                .lease(identity, BranchPhase::Rollback)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        fixture::save(
            "java-armed-child-recovered.json",
            &serde_json::json!({
                "xid": xid.as_str(), "branch_id": branch_id, "resource_id": key.resource_id(),
                "recovery_outcome": if arming_blocked {"MissingAttempt"} else {"NotAppliedFenced"},
                "observed_end":observed_end,
                "fenced_before_global_rollback":before
            }),
        );
        rm.shutdown().await.unwrap();
        tm.shutdown().await.unwrap();
        ds.close().await.unwrap();
        return;
    }
    let mut race_handoff: Vec<TcpStream> = Vec::new();
    let mut race_children = Vec::new();
    let mut race_child_pids = Vec::new();
    let mut race_outcomes = Vec::new();
    if distinct_processes {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        for _ in 0..2 {
            let child = Command::new(std::env::current_exe().unwrap())
                .arg("original_java_tc_report_race_child")
                .arg("--exact")
                .arg("--ignored")
                .arg("--nocapture")
                .arg("--test-threads=1")
                .env("SEATA_JAVA_TC_REPORT_RACE_CHILD", "1")
                .env("SEATA_JAVA_TC_COMMIT_XID", xid.as_str())
                .env("SEATA_JAVA_TC_COMMIT_BRANCH", branch_id.to_string())
                .env(
                    "SEATA_JAVA_TC_REPORT_RACE_BARRIER",
                    listener.local_addr().unwrap().to_string(),
                )
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            race_child_pids.push(child.id());
            race_children.push(child);
        }
        for _ in 0..2 {
            let (socket, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
                .await
                .unwrap()
                .unwrap();
            race_handoff.push(socket);
        }
        // 两个恢复者已独立启动，释放同一屏障后才允许争抢 007 门闩。
        for socket in &mut race_handoff {
            socket.write_all(&[1]).await.unwrap();
        }
        for socket in &mut race_handoff {
            let mut outcome = [0_u8; 1];
            tokio::time::timeout(Duration::from_secs(10), socket.read_exact(&mut outcome))
                .await
                .unwrap()
                .unwrap();
            race_outcomes.push(outcome[0]);
        }
        race_outcomes.sort_unstable();
        assert_eq!(race_outcomes, [1, 2]);
        assert_eq!(race_child_pids.len(), 2);
        assert_ne!(race_child_pids[0], race_child_pids[1]);
        assert!(race_child_pids.iter().all(|pid| *pid != std::process::id()));
    } else if concurrent {
        let second_ds = MySqlDataSource::connect(
            owned_mysql::options(None).with_java_tc_mysql_db_lock_profile(),
        )
        .await
        .unwrap();
        let second =
            MySqlDataSourceManager::new(second_ds.clone(), MySqlJdbcProfile::Connector8Default)
                .unwrap();
        let client_a = rm.client();
        let client_b = rm.client();
        let (a, b) = tokio::join!(
            adapter.report_recovered_applied(&client_a, xid.as_str(), branch_id),
            second.report_recovered_applied(&client_b, xid.as_str(), branch_id),
        );
        let loser = match (a, b) {
            (Ok(_), Err(error)) | (Err(error), Ok(_)) => error,
            outcomes => panic!("expected exactly one recovered report: {outcomes:?}"),
        };
        assert!(matches!(
            loser,
            seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted
        ));
        second_ds.close().await.unwrap();
    } else {
        adapter
            .report_recovered_applied(&rm.client(), xid.as_str(), branch_id)
            .await
            .unwrap();
    }
    if !distinct_processes {
        let duplicate = adapter
            .report_recovered_applied(&rm.client(), xid.as_str(), branch_id)
            .await
            .unwrap_err();
        assert!(matches!(
            duplicate,
            seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted
        ));
    }
    let report_started: u8 = sqlx::query_scalar(
        "SELECT report_started FROM seata_rs_at_commit_attempt WHERE xid=? AND branch_id=?",
    )
    .bind(xid.as_str().as_bytes())
    .bind(branch_id)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(report_started, 1);
    let before: (i64, i64, u8, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM undo_work), (SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0), (SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?), (SELECT COUNT(*) FROM lock_table WHERE xid=? AND branch_id=? AND pk='1')",
    )
    .bind(xid.as_str())
    .bind(branch_id)
    .bind(xid.as_str().as_bytes())
    .bind(branch_id)
    .bind(xid.as_str())
    .bind(branch_id)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(before, (1, 1, 1, 1));
    match global.commit().await.unwrap() {
        TransactionCompletion::Observed { outcome, .. } => {
            assert_eq!(outcome.status(), GlobalStatus::Committed)
        }
        other => panic!("{other:?}"),
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let row = sqlx::query("SELECT first_phase_committed,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?")
            .bind(xid.as_str().as_bytes())
            .bind(branch_id)
            .fetch_one(&mut admin)
            .await
            .unwrap();
        if (
            row.get::<u8, _>("first_phase_committed"),
            row.get::<u8, _>("commit_authorized"),
            row.get::<u8, _>("commit_terminal"),
        ) == (1, 1, 1)
        {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    if !distinct_processes {
        let branch_identity =
            BranchIdentity::new(xid.as_str().into(), branch_id, key.clone(), None).unwrap();
        assert_eq!(
            store
                .lease(branch_identity, BranchPhase::Commit)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
    }
    loop {
        if tm.get_status(&xid).await.unwrap().status() == GlobalStatus::Finished {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    for socket in &mut race_handoff {
        socket.write_all(&[2]).await.unwrap();
    }
    for child in race_children {
        let pid = child.id();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "recovery child {pid}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fixture::save(
        if distinct_processes {
            "java-distinct-process-race.json"
        } else if concurrent {
            "java-cross-process-race.json"
        } else if ack_held {
            "java-applied-child-recovered.json"
        } else {
            "java-cross-process-recovered.json"
        },
        &serde_json::json!({"xid":xid.as_str(),"branch_id":branch_id,"resource_id":key.resource_id(),"before_commit":{"business":before.0,"normal_undo":before.1,"witness":before.2,"tc_lock":before.3},"attempt_rows":1,"report_response":"Success","recovery_outcome":"Applied","child_exited_before_recovery":!competing,"concurrent_recovery_callers":concurrent,"distinct_recovery_processes":distinct_processes,"recovery_child_pids":race_child_pids,"recovery_claim_outcomes":race_outcomes,"local_commit_confirmed":true,"original_receipt_preserved":false,"global_result":"Committed","callback":"Completed","business_rows":1,"undo_rows":0,"first_phase_committed":1}),
    );
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_rolled_back_local_branch_reports_phase_one_failed() {
    original_java_tc_failed_report(false).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_lost_failure_report_reply_keeps_rollback_and_receipt() {
    original_java_tc_failed_report(true).await;
}

#[cfg(feature = "experimental_changed_commit")]
async fn original_java_tc_failed_report(lose_reply: bool) {
    let mut admin = fixture::reset().await;
    for migration in [
        include_str!("../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/005_seata_rs_at_resource_binding.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    let ds =
        MySqlDataSource::connect(owned_mysql::options(None).with_java_tc_mysql_db_lock_profile())
            .await
            .unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(ds.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let key = adapter.key().clone();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(key.resource_id().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let store = FileBranchStore::open(
        folder.join("java-failed-report-branches.log"),
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
        store,
    )
    .request_timeout(Duration::from_secs(3))
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
    let cfg = TransactionConfig::builder("mysql-failed-report-original-tc".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut global = tm.begin(&cfg).await.unwrap();
    let xid = global.xid().clone();
    let mut local = ds.begin().await.unwrap();
    local
        .execute(
            &SqlRecognizerFactory::recognize(
                "INSERT INTO undo_work(id,v) VALUES(1,'original_tc_failed_report')",
            )
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
    let prepared = adapter
        .prepare_transaction(local, &rm.client(), xid.as_str(), None)
        .await
        .unwrap();
    let branch_id = prepared.registered_branch().unwrap().identity().branch_id();
    let before: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM undo_work), (SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=?), (SELECT COUNT(*) FROM lock_table WHERE xid=? AND branch_id=? AND pk='1')",
    )
    .bind(xid.as_str())
    .bind(branch_id)
    .bind(xid.as_str())
    .bind(branch_id)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(before, (0, 0, 1));
    let original = prepared.registered_branch().unwrap().clone();
    if lose_reply {
        let error = prepared
            .rollback_and_report_failure(&rm.client())
            .await
            .unwrap_err();
        assert!(error.local_rollback_confirmed());
        assert!(error.is_unknown_outcome());
        assert_eq!(
            error.registered_branch().unwrap().identity(),
            original.identity()
        );
    } else {
        let reported = prepared
            .rollback_and_report_failure(&rm.client())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reported.identity().branch_id(), branch_id);
    }
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    match global.rollback().await.unwrap() {
        TransactionCompletion::Observed { outcome, .. } => {
            assert_eq!(outcome.status(), GlobalStatus::Rollbacked)
        }
        other => panic!("{other:?}"),
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if tm.get_status(&xid).await.unwrap().status() == GlobalStatus::Finished {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC cleanup required"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(fixture::count(&mut admin, "lock_table").await, 0);
    let witness: (u8, u8) = sqlx::query_as(
        "SELECT first_phase_committed,commit_terminal FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?",
    )
    .bind(xid.as_str().as_bytes())
    .bind(branch_id)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(witness, (0, 0));
    fixture::save(
        if lose_reply {
            "java-failed-report-loss.json"
        } else {
            "java-failed-report.json"
        },
        &serde_json::json!({"xid":xid.as_str(),"branch_id":branch_id,"resource_id":key.resource_id(),"before_report":{"business":before.0,"normal_undo":before.1,"tc_lock":before.2},"report_response":if lose_reply {"Lost"} else {"Success"},"local_rollback_confirmed":true,"original_receipt_preserved":true,"global_result":"Rollbacked","business_rows":0,"undo_rows":0,"first_phase_committed":0,"commit_terminal":0}),
    );
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_undo_table_guard_error_rolls_back_before_automatic_failure_report() {
    original_java_tc_prepare_error_reports("Undo", false).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_undo_guard_error_lost_failure_report_reply_preserves_prepare_error() {
    original_java_tc_prepare_error_reports("Undo", true).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_enrollment_error_rolls_back_before_automatic_failure_report() {
    original_java_tc_prepare_error_reports("Enrollment", false).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_real_undo_unique_conflict_rolls_back_before_failure_report() {
    original_java_tc_prepare_error_reports("UndoDuplicate", false).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_real_enrollment_unique_conflict_rolls_back_before_failure_report() {
    original_java_tc_prepare_error_reports("EnrollmentDuplicate", false).await;
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires pinned original Java TC, raw observers and owned MySQL DB lock store"]
async fn original_java_tc_enrollment_unique_conflict_lost_failure_report_reply() {
    original_java_tc_prepare_error_reports("EnrollmentDuplicate", true).await;
}

#[cfg(feature = "experimental_changed_commit")]
async fn original_java_tc_prepare_error_reports(failure_kind: &str, lose_reply: bool) {
    use seata_db_mysql::{
        DbError, MySqlPrepareError, MySqlPrepareFailureReportError, MySqlRecoveryError,
    };

    assert!(!lose_reply || matches!(failure_kind, "Undo" | "EnrollmentDuplicate"));

    let mut admin = fixture::reset().await;
    for migration in [
        include_str!("../migrations/001_seata_rs_at_branch_v1.sql"),
        include_str!("../migrations/002_seata_rs_at_registration_origin.sql"),
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/005_seata_rs_at_resource_binding.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut admin).await.unwrap();
    }
    if !matches!(failure_kind, "UndoDuplicate" | "EnrollmentDuplicate") {
        let trigger = if failure_kind == "Undo" {
            "CREATE TRIGGER reject_normal_undo BEFORE INSERT ON undo_log FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='fixture rejects original Undo insertion'"
        } else {
            assert_eq!(failure_kind, "Enrollment");
            "CREATE TRIGGER reject_control_enrollment BEFORE INSERT ON seata_rs_at_branch_v1 FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='fixture rejects control enrollment'"
        };
        sqlx::raw_sql(trigger).execute(&mut admin).await.unwrap();
    }
    let options = owned_mysql::options(None).with_java_tc_mysql_db_lock_profile();
    let options = if matches!(failure_kind, "UndoDuplicate" | "EnrollmentDuplicate") {
        options.operation_timeout(Duration::from_secs(20)).unwrap()
    } else {
        options
    };
    let ds = MySqlDataSource::connect(options).await.unwrap();
    let adapter = Arc::new(
        MySqlDataSourceManager::new(ds.clone(), MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let key = adapter.key().clone();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(key.resource_id().as_bytes())
        .execute(&mut admin)
        .await
        .unwrap();
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let store = FileBranchStore::open(
        folder.join(format!("java-{failure_kind}-error-branches.log")),
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
        store,
    )
    .request_timeout(Duration::from_secs(3))
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
    let cfg = TransactionConfig::builder("mysql-undo-error-original-tc".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut global = tm.begin(&cfg).await.unwrap();
    let xid = global.xid().clone();
    let mut local = ds.begin().await.unwrap();
    local
        .execute(
            &SqlRecognizerFactory::recognize(
                "INSERT INTO undo_work(id,v) VALUES(1,'original_tc_undo_error')",
            )
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
    let mut progress = None;
    let client = rm.client();
    let (error, injected_branch) = if matches!(
        failure_kind,
        "UndoDuplicate" | "EnrollmentDuplicate"
    ) {
        // 控制登记暂时等待，本地业务锁仍由原事务持有；先取得 TC 的真实编号再造目标表唯一键冲突。
        sqlx::raw_sql("LOCK TABLES seata_rs_at_branch_v1 WRITE")
            .execute(&mut admin)
            .await
            .unwrap();
        let mut observer = owned_mysql::admin().await;
        let mut pending = Box::pin(adapter.prepare_transaction_and_report_failure(
            local,
            &client,
            xid.as_str(),
            None,
            &mut progress,
        ));
        let waiting_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let original_branch = loop {
            tokio::select! {
                result = &mut pending => panic!("prepare completed before original branch became observable: {result:?}"),
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
            if let Some(branch_id) =
                sqlx::query_scalar::<_, i64>("SELECT branch_id FROM branch_table WHERE xid=?")
                    .bind(xid.as_str())
                    .fetch_optional(&mut observer)
                    .await
                    .unwrap()
            {
                break branch_id;
            }
            assert!(tokio::time::Instant::now() < waiting_deadline);
        };
        if failure_kind == "UndoDuplicate" {
            sqlx::query("INSERT INTO undo_log (branch_id,xid,context,rollback_info,log_status,log_created,log_modified) VALUES (?,?,'serializer=jackson&compressorType=NONE',?,1,now(6),now(6))")
                .bind(original_branch)
                .bind(xid.as_str())
                .bind(b"{}".as_slice())
                .execute(&mut observer)
                .await
                .unwrap();
        } else {
            // 同一真实编号的回调归属行先落库；原登记 INSERT 必须实际撞唯一键且不得覆盖此行。
            sqlx::query("INSERT INTO seata_rs_at_branch_v1(schema_version,xid,branch_id,resource_id,registration_origin,jdbc_profile,phase,terminal,created_at,modified_at) VALUES(3,?,?,?,2,1,2,1,now(6),now(6))")
                .bind(xid.as_str().as_bytes())
                .bind(original_branch)
                .bind(key.resource_id().as_bytes())
                .execute(&mut admin)
                .await
                .unwrap();
        }
        sqlx::raw_sql("UNLOCK TABLES")
            .execute(&mut admin)
            .await
            .unwrap();
        let error = pending.await.unwrap_err();
        (error, Some(original_branch))
    } else {
        let error = adapter
            .prepare_transaction_and_report_failure(
                local,
                &client,
                xid.as_str(),
                None,
                &mut progress,
            )
            .await
            .unwrap_err();
        (error, None)
    };
    assert!(matches!(
        (failure_kind, error.prepare_error()),
        ("Undo", MySqlPrepareError::Undo { .. })
            | ("UndoDuplicate", MySqlPrepareError::Undo { .. })
            | ("Enrollment", MySqlPrepareError::Enrollment { .. })
            | ("EnrollmentDuplicate", MySqlPrepareError::Enrollment { .. })
    ));
    if failure_kind == "Undo" {
        assert!(matches!(
            error.prepare_error(),
            MySqlPrepareError::Undo {
                source: DbError::Unsupported {
                    feature: "Undo trigger or foreign key"
                },
                ..
            }
        ));
    }
    if failure_kind == "UndoDuplicate" {
        assert!(matches!(
            error.prepare_error(),
            MySqlPrepareError::Undo {
                source: DbError::Driver {
                    operation: "Undo insert",
                    sql_state: Some(state),
                    code: Some(1062),
                    ..
                },
                ..
            } if state == "23000"
        ));
    }
    if failure_kind == "EnrollmentDuplicate" {
        assert!(matches!(
            error.prepare_error(),
            MySqlPrepareError::Enrollment {
                source: MySqlRecoveryError::BeforeCommit {
                    source: DbError::CallbackObserved,
                    ..
                },
                ..
            }
        ));
    }
    assert!(error.prepare_error().local_rollback_confirmed());
    assert!(error.prepare_error().failure_report_started());
    if lose_reply {
        assert!(error
            .report_result()
            .unwrap()
            .as_ref()
            .unwrap_err()
            .is_unknown_outcome());
        assert!(matches!(
            error
                .prepare_error()
                .report_failure_after_rollback(&client)
                .await,
            Err(MySqlPrepareFailureReportError::AlreadyAttempted)
        ));
    } else {
        assert!(error.report_result().unwrap().is_ok());
    }
    assert!(progress.is_none());
    let branch_id = error
        .prepare_error()
        .registered_branch()
        .unwrap()
        .identity()
        .branch_id();
    if let Some(original_branch) = injected_branch {
        assert_eq!(branch_id, original_branch);
    }
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    let expected_undo_rows = i64::from(failure_kind == "UndoDuplicate");
    assert_eq!(
        fixture::count(&mut admin, "undo_log").await,
        expected_undo_rows
    );
    if failure_kind == "EnrollmentDuplicate" {
        let row: (i64, u8, u8, u8) = sqlx::query_as(
            "SELECT branch_id,registration_origin,phase,terminal FROM seata_rs_at_branch_v1 WHERE xid=?",
        )
        .bind(xid.as_str().as_bytes())
        .fetch_one(&mut admin)
        .await
        .unwrap();
        assert_eq!(row, (branch_id, 2, 2, 1));
        // 测试预置行不属于真实回调，故在 TC 发起 GlobalRollback 前撤销夹具污染。
        sqlx::query("DELETE FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?")
            .bind(xid.as_str().as_bytes())
            .bind(branch_id)
            .execute(&mut admin)
            .await
            .unwrap();
    }
    match global.rollback().await.unwrap() {
        TransactionCompletion::Observed { outcome, .. } => {
            assert_eq!(outcome.status(), GlobalStatus::Rollbacked)
        }
        other => panic!("{other:?}"),
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        if tm.get_status(&xid).await.unwrap().status() == GlobalStatus::Finished {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC cleanup required"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(fixture::count(&mut admin, "lock_table").await, 0);
    let control_rows = fixture::count(&mut admin, "seata_rs_at_branch_v1").await;
    if !matches!(failure_kind, "Enrollment" | "EnrollmentDuplicate") {
        let branch: (i64, u8, u8) = sqlx::query_as(
            "SELECT branch_id,first_phase_committed,commit_terminal FROM seata_rs_at_branch_v1 WHERE xid=?",
        )
        .bind(xid.as_str().as_bytes())
        .fetch_one(&mut admin)
        .await
        .unwrap();
        assert_eq!(branch, (branch_id, 0, 0));
        assert_eq!(control_rows, 1);
    } else {
        assert_eq!(control_rows, 0);
    }
    let observation_name = match failure_kind {
        "Undo" if lose_reply => "java-prepare-undo-report-loss.json",
        "Undo" => "java-prepare-undo-failed.json",
        "Enrollment" => "java-prepare-enrollment-failed.json",
        "EnrollmentDuplicate" if lose_reply => "java-prepare-enrollment-duplicate-report-loss.json",
        "EnrollmentDuplicate" => "java-prepare-enrollment-duplicate.json",
        "UndoDuplicate" => "java-prepare-undo-duplicate.json",
        _ => unreachable!(),
    };
    fixture::save(
        observation_name,
        &serde_json::json!({"xid":xid.as_str(),"branch_id":branch_id,"resource_id":key.resource_id(),"prepare_error":failure_kind,"local_rollback_confirmed":true,"failure_report_started":true,"report_result":if lose_reply {"Lost"} else {"Success"},"original_receipt_preserved":true,"report_replay_rejected":lose_reply,"global_result":"Rollbacked","business_rows":0,"undo_rows":expected_undo_rows,"control_rows":control_rows,"first_phase_committed":0,"commit_terminal":0,"driver_code":if matches!(failure_kind, "UndoDuplicate" | "EnrollmentDuplicate") {Some(1062)} else {None}}),
    );
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    ds.close().await.unwrap();
}
