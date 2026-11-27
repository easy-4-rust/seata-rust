//! 原业务事务中的提交前仲裁；默认构建没有有变化业务 COMMIT 入口。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
#[allow(dead_code)]
mod callback_peer;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/failure_report_metrics_cases.rs"]
mod failure_report_metrics_cases;
#[path = "support/first_phase_fixture.rs"]
mod first_fixture;
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/managed_rollback_fence_cases.rs"]
mod managed_rollback_fence_cases;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[cfg(feature = "experimental_changed_commit")]
#[path = "support/first_phase_metrics_cases.rs"]
mod phase_metrics_cases;
#[path = "support/rm_recovery_fixture.rs"]
#[allow(dead_code)]
mod recovery;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;
#[path = "support/first_phase_witness_cases.rs"]
mod witness_cases;

use first_fixture::Harness;
#[cfg(feature = "experimental_changed_commit")]
use seata_db_mysql::MySqlChangedCommitError;
use seata_db_mysql::{DbError, MySqlPreparedTransaction};
#[cfg(feature = "experimental_changed_commit")]
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager};
#[cfg(feature = "experimental_changed_commit")]
use seata_protocol::transaction::BranchReportResponse;
#[cfg(feature = "experimental_changed_commit")]
use seata_protocol::Message;
use seata_protocol::{BranchStatus, ResultCode};
#[cfg(feature = "experimental_changed_commit")]
use seata_rm::RegisteredBranch;
use seata_rm::{BranchPhase, BranchResource};
#[cfg(feature = "experimental_changed_commit")]
use sqlx::Row;
#[cfg(feature = "experimental_changed_commit")]
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpStream;

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn legacy_commit_attempt_migration_conservatively_marks_report_started() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    sqlx::raw_sql(include_str!(
        "../migrations/006_seata_rs_at_commit_attempt.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    sqlx::query("INSERT INTO seata_rs_at_commit_attempt (schema_version,xid,branch_id,resource_id,jdbc_profile,response_result_code,response_exception_code,attempt_state,created_at,modified_at) VALUES(1,?,42,?,1,1,0,1,now(6),now(6))")
        .bind(b"legacy:possibly-reported".as_slice())
        .bind(h.adapter.key().resource_id().as_bytes())
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/007_seata_rs_at_commit_report_started.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    let started: u8 = sqlx::query_scalar("SELECT report_started FROM seata_rs_at_commit_attempt")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(started, 1);
    h.rm.shutdown().await.unwrap();
    h.source.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_persists_business_undo_and_witness_together() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:atomic").await;
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    assert!(checked.require_first_phase_commit_witness().is_ok());
    sqlx::query("SET GLOBAL log_output='TABLE'")
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=ON")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    let receipt = checked.commit_changed(&h.adapter).await.unwrap();
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(
        after,
        before + 2,
        "one durable arming COMMIT and one original business COMMIT required"
    );
    assert_eq!(receipt.identity().branch_id(), 42);
    assert!(h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    assert_eq!(
        h.adapter
            .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
            .await
            .unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::Applied
    );
    let witness: u8 = sqlx::query_scalar("SELECT first_phase_committed FROM seata_rs_at_branch_v1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(witness, 1);
    let attempt = sqlx::query(
        "SELECT xid,branch_id,resource_id,application_data,lock_key,jdbc_profile,attempt_state FROM seata_rs_at_commit_attempt",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(attempt.get::<Vec<u8>, _>("xid"), b"commit-changed:atomic");
    assert_eq!(
        attempt.get::<i64, _>("branch_id"),
        receipt.identity().branch_id()
    );
    assert_eq!(
        attempt.get::<Vec<u8>, _>("resource_id"),
        receipt.identity().resource().resource_id().as_bytes()
    );
    assert_eq!(
        attempt
            .get::<Option<Vec<u8>>, _>("application_data")
            .as_deref(),
        Some(b"prepare-fixture-app".as_slice())
    );
    assert_eq!(
        attempt.get::<Option<Vec<u8>>, _>("lock_key").as_deref(),
        Some(b"undo_work:1".as_slice())
    );
    assert_eq!(
        (
            attempt.get::<u8, _>("jdbc_profile"),
            attempt.get::<u8, _>("attempt_state")
        ),
        (1, 1)
    );
    let reopened = MySqlDataSourceManager::new(
        MySqlDataSource::connect(
            owned_mysql::options(None)
                .operation_timeout(Duration::from_secs(5))
                .unwrap(),
        )
        .await
        .unwrap(),
        seata_at::MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    let recovered = reopened
        .load_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.identity(), receipt.identity());
    assert_eq!(recovered.lock_key(), receipt.lock_key());
    assert_eq!(
        recovered.profile(),
        seata_at::MySqlJdbcProfile::Connector8Default
    );
    assert!(reopened.commit_attempt_observed(&recovered).await.unwrap());
    assert_eq!(
        reopened
            .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
            .await
            .unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::Applied
    );
    // 有 Normal Undo 但见证被破坏时，恢复不能把暂缺见证发布为 NotAppliedFenced。
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET first_phase_committed=0")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(reopened
        .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
        .await
        .unwrap_err()
        .is_unknown_outcome());
    let normal: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=0",
    )
    .bind(receipt.identity().xid())
    .bind(receipt.identity().branch_id())
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(normal, 1);
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET first_phase_committed=1")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(tokio::time::timeout(
        Duration::from_millis(60),
        rm_fixture::peer::read(&mut socket)
    )
    .await
    .is_err());
    sqlx::query("UPDATE seata_rs_at_commit_attempt SET resource_id=?")
        .bind(b"changed-resource".as_slice())
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(matches!(
        reopened
            .load_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
            .await,
        Err(DbError::RecoveryConflict)
    ));
    assert!(reopened
        .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
        .await
        .unwrap_err()
        .is_unknown_outcome());
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CommitChanged,
        seata_db_mysql::MySqlPhaseOneOutcome::ChangedCommitted,
        1,
    );
    assert_eq!(h.closed_counts(&mut socket).await, (1, 1, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_rejects_missing_attempt_migration_before_sql_commit() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:missing-attempt").await;
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    let error = checked.commit_changed(&h.adapter).await.unwrap_err();
    assert!(matches!(
        error,
        MySqlChangedCommitError::Arming {
            rollback: Ok(()),
            ..
        }
    ));
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CommitChanged,
        seata_db_mysql::MySqlPhaseOneOutcome::ArmingFailed,
        1,
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_rejects_missing_report_gate_before_sql_commit() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:missing-report-gate").await;
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    assert!(matches!(
        checked.commit_changed(&h.adapter).await.unwrap_err(),
        MySqlChangedCommitError::Arming {
            rollback: Ok(()),
            ..
        }
    ));
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_rejects_nontransactional_attempt_table() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    sqlx::raw_sql("ALTER TABLE seata_rs_at_commit_attempt ENGINE=MyISAM")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let (prepared, mut socket) = prepared(&h, "commit-changed:unsafe-attempt").await;
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    assert!(matches!(
        checked.commit_changed(&h.adapter).await.unwrap_err(),
        MySqlChangedCommitError::Arming {
            rollback: Ok(()),
            ..
        }
    ));
    assert_eq!(
        fixture::count(&mut h.admin, "seata_rs_at_commit_attempt").await,
        0
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_rejects_conflicting_armed_identity() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:conflicting-attempt").await;
    let receipt = prepared.registered_branch().unwrap();
    sqlx::query("INSERT INTO seata_rs_at_commit_attempt (schema_version,xid,branch_id,resource_id,application_data,lock_key,jdbc_profile,response_result_code,response_exception_code,attempt_state,report_started,created_at,modified_at) VALUES(1,?,?,'different-resource',?,?,1,1,0,1,0,now(6),now(6))")
        .bind(receipt.identity().xid().as_bytes())
        .bind(receipt.identity().branch_id())
        .bind(receipt.identity().application_data().map(str::as_bytes))
        .bind(receipt.lock_key().map(str::as_bytes))
        .execute(&mut h.admin)
        .await
        .unwrap();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    assert!(matches!(
        checked.commit_changed(&h.adapter).await.unwrap_err(),
        MySqlChangedCommitError::Arming {
            rollback: Ok(()),
            ..
        }
    ));
    let resource: Vec<u8> =
        sqlx::query_scalar("SELECT resource_id FROM seata_rs_at_commit_attempt")
            .fetch_one(&mut h.admin)
            .await
            .unwrap();
    assert_eq!(resource, b"different-resource");
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_rejects_legacy_witness_before_sending() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    sqlx::raw_sql(include_str!(
        "../migrations/003_seata_rs_at_commit_decision.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    let (prepared, mut socket) = prepared(&h, "commit-changed:legacy").await;
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    let error = checked.commit_changed(&h.adapter).await.unwrap_err();
    assert!(!error.is_unknown_outcome());
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CheckCommit,
        seata_db_mysql::MySqlPhaseOneOutcome::CheckedWithoutWitness,
        1,
    );
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CommitChanged,
        seata_db_mysql::MySqlPhaseOneOutcome::ChangedNotEligible,
        1,
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_driver_loss_retains_unknown_receipt() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:driver-loss").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    let owner: Vec<u64> = sqlx::query_scalar("SELECT DISTINCT THREAD_ID FROM performance_schema.data_locks WHERE OBJECT_SCHEMA=DATABASE() AND OBJECT_NAME='undo_work' AND LOCK_TYPE='TABLE'")
        .fetch_all(&mut h.admin).await.unwrap();
    assert_eq!(owner.len(), 1);
    let connection_id: u64 = sqlx::query_scalar(
        "SELECT PROCESSLIST_ID FROM performance_schema.threads WHERE THREAD_ID=?",
    )
    .bind(owner[0])
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    sqlx::query(&format!("KILL CONNECTION {connection_id}"))
        .execute(&mut h.admin)
        .await
        .unwrap();
    let error = checked
        .commit_changed_and_report(&h.adapter, &h.rm.client())
        .await
        .unwrap_err();
    assert!(error.is_unknown_outcome());
    assert!(!error.local_commit_confirmed());
    assert_eq!(
        error.registered_branch().unwrap().identity(),
        receipt.identity()
    );
    assert!(!h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    let recovered = h
        .adapter
        .load_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
        .await
        .unwrap()
        .unwrap();
    assert!(!h.adapter.commit_attempt_observed(&recovered).await.unwrap());
    let witness: u8 = sqlx::query_scalar("SELECT first_phase_committed FROM seata_rs_at_branch_v1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(witness, 0);
    assert!(tokio::time::timeout(
        Duration::from_millis(60),
        rm_fixture::peer::read(&mut socket)
    )
    .await
    .is_err());
    let reopened = MySqlDataSourceManager::new(
        MySqlDataSource::connect(
            owned_mysql::options(None)
                .operation_timeout(Duration::from_secs(5))
                .unwrap(),
        )
        .await
        .unwrap(),
        seata_at::MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CommitChanged,
        seata_db_mysql::MySqlPhaseOneOutcome::ChangedCommitUnknown,
        1,
    );
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CommitAndReport,
        seata_db_mysql::MySqlPhaseOneOutcome::ReportLocalUncertain,
        1,
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
    assert_eq!(
        reopened
            .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
            .await
            .unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::NotAppliedFenced
    );
    let mut independent = owned_mysql::admin().await;
    let guard: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=1 AND rollback_info=?",
    )
    .bind(receipt.identity().xid())
    .bind(receipt.identity().branch_id())
    .bind(b"{}".as_slice())
    .fetch_one(&mut independent)
    .await
    .unwrap();
    assert_eq!(guard, 1);
    let witness: u8 = sqlx::query_scalar(
        "SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?",
    )
    .bind(receipt.identity().xid().as_bytes())
    .bind(receipt.identity().branch_id())
    .fetch_one(&mut independent)
    .await
    .unwrap();
    assert_eq!(witness, 0);
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn armed_recovery_waits_for_old_undo_owner_before_fencing() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:old-owner").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    let id = receipt.identity();
    arm_for_recovery_fixture(&mut h.admin, &receipt).await;
    let brief = MySqlDataSourceManager::new(
        MySqlDataSource::connect(
            owned_mysql::options(None)
                .operation_timeout(Duration::from_millis(800))
                .unwrap(),
        )
        .await
        .unwrap(),
        seata_at::MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    let unresolved = brief.recover_commit_attempt(id.xid(), id.branch_id()).await;
    assert!(unresolved.unwrap_err().is_unknown_outcome());
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    checked.rollback().await.unwrap();
    assert_eq!(
        h.adapter
            .recover_commit_attempt(id.xid(), id.branch_id())
            .await
            .unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::NotAppliedFenced
    );
    let guard: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=1",
    )
    .bind(id.xid())
    .bind(id.branch_id())
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(guard, 1);
    assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn armed_recovery_query_failure_is_unknown_without_guard() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:query-failure").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    arm_for_recovery_fixture(&mut h.admin, &receipt).await;
    checked.rollback().await.unwrap();
    let closed_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let closed_manager = MySqlDataSourceManager::new(
        closed_source.clone(),
        seata_at::MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    closed_source.close().await.unwrap();
    assert!(closed_manager
        .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
        .await
        .unwrap_err()
        .is_unknown_outcome());
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(
        h.adapter
            .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
            .await
            .unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::NotAppliedFenced
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn armed_recovery_missing_witness_row_is_unknown_without_guard() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:missing-witness-row").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    arm_for_recovery_fixture(&mut h.admin, &receipt).await;
    checked.rollback().await.unwrap();
    sqlx::query("DELETE FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?")
        .bind(receipt.identity().xid().as_bytes())
        .bind(receipt.identity().branch_id())
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(h
        .adapter
        .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
        .await
        .unwrap_err()
        .is_unknown_outcome());
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
async fn arm_for_recovery_fixture(admin: &mut sqlx::MySqlConnection, receipt: &RegisteredBranch) {
    let id = receipt.identity();
    // 仅在测试夹具模拟“独立 Armed 已提交、原业务事务尚未发送 COMMIT”的中断点。
    sqlx::query(
        "INSERT INTO seata_rs_at_commit_attempt \
         (schema_version,xid,branch_id,resource_id,application_data,lock_key,jdbc_profile,\
          response_result_code,response_exception_code,attempt_state,report_started,created_at,modified_at) \
         VALUES(1,?,?,?,?,?,?,?,?,1,0,now(6),now(6))",
    )
    .bind(id.xid().as_bytes())
    .bind(id.branch_id())
    .bind(id.resource().resource_id().as_bytes())
    .bind(id.application_data().map(str::as_bytes))
    .bind(receipt.lock_key().map(str::as_bytes))
    .bind(1_u8)
    .bind(receipt.response().response.result_code.to_java_v1())
    .bind(
        receipt
            .response()
            .response
            .transaction_exception_code
            .to_java_v1(),
    )
    .execute(admin)
    .await
    .unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn armed_recovery_waits_for_old_commit_and_observes_applied() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:old-commit").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    arm_for_recovery_fixture(&mut h.admin, &receipt).await;
    let reopened = MySqlDataSourceManager::new(
        MySqlDataSource::connect(
            owned_mysql::options(None)
                .operation_timeout(Duration::from_secs(5))
                .unwrap(),
        )
        .await
        .unwrap(),
        seata_at::MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    let xid = receipt.identity().xid().to_owned();
    let branch_id = receipt.identity().branch_id();
    let mut recovery =
        tokio::spawn(async move { reopened.recover_commit_attempt(&xid, branch_id).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(150), &mut recovery)
            .await
            .is_err()
    );
    checked.commit_changed(&h.adapter).await.unwrap();
    assert_eq!(
        recovery.await.unwrap().unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::Applied
    );
    let guard: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=1",
    )
    .bind(receipt.identity().xid())
    .bind(receipt.identity().branch_id())
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(guard, 0);
    assert_eq!(h.closed_counts(&mut socket).await, (1, 1, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "child process controlled by armed_crash_parent_recovers_original_attempt"]
async fn armed_crash_child_holds_original_transaction() {
    let Ok(marker) = std::env::var("SEATA_COMMIT_CRASH_MARKER") else {
        return;
    };
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, _socket) = prepared(&h, "commit-changed:crash-child").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let _checked = prepared.lock_commit_preconditions().await.unwrap();
    arm_for_recovery_fixture(&mut h.admin, &receipt).await;
    std::fs::write(marker, b"armed-and-original-transaction-open").unwrap();
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture and isolated test child"]
async fn armed_crash_parent_recovers_original_attempt() {
    use std::process::{Command, Stdio};
    let output = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    assert_eq!(output.to_string_lossy().chars().next(), Some('D'));
    let marker = output.join(format!("armed-crash-marker-{}.txt", std::process::id()));
    assert!(!marker.exists());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("armed_crash_child_holds_original_transaction")
        .arg("--exact")
        .arg("--ignored")
        .arg("--test-threads=1")
        .env("SEATA_COMMIT_CRASH_MARKER", &marker)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut armed = false;
    for _ in 0..100 {
        if marker.exists() {
            armed = true;
            break;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "child exited before arming"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !armed {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("child did not persist Armed before deadline");
    }
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    println!("owned Armed child pid={} exited={status}", child.id());
    std::fs::remove_file(&marker).unwrap();
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let recovered =
        MySqlDataSourceManager::new(source, seata_at::MySqlJdbcProfile::Connector8Default).unwrap();
    let xid = "commit-changed:crash-child";
    assert_eq!(
        recovered.recover_commit_attempt(xid, 42).await.unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::NotAppliedFenced
    );
    let mut admin = owned_mysql::admin().await;
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    let guard: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=42 AND log_status=1",
    )
    .bind(xid)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(guard, 1);
    let witness: u8 = sqlx::query_scalar(
        "SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42",
    )
    .bind(xid.as_bytes())
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(witness, 0);
    println!("new process recovery: business=0 guard={guard} witness={witness}");
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "child process controlled by armed_crash_after_commit_parent_observes_applied"]
async fn armed_crash_after_commit_child() {
    if std::env::var("SEATA_COMMIT_CRASH_AFTER_COMMIT_CHILD").as_deref() != Ok("1") {
        return;
    }
    let proxy_port: u16 = std::env::var("SEATA_MYSQL_COMMIT_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let arm_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_ARM").unwrap());
    let mut h =
        Harness::start_with_source_port(Duration::from_secs(4), Duration::from_secs(2), proxy_port)
            .await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, _socket) = prepared(&h, "commit-changed:crash-after-commit").await;
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    std::fs::write(&arm_file, b"armed\n").unwrap();
    // 代理先放行 006 控制 COMMIT，再扣留原业务 COMMIT 的服务器 OK。
    let _ = checked.commit_changed(&h.adapter).await;
    panic!("child must be stopped while original business COMMIT OK is held");
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL and proxy holding original business COMMIT OK"]
async fn armed_crash_after_commit_parent_observes_applied() {
    use std::process::{Command, Stdio};
    let proxy_port: u16 = std::env::var("SEATA_MYSQL_COMMIT_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let arm_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_ARM").unwrap());
    let ok_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_OK").unwrap());
    assert!(!arm_file.exists() && !ok_file.exists());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("armed_crash_after_commit_child")
        .arg("--exact")
        .arg("--ignored")
        .arg("--test-threads=1")
        .env("SEATA_COMMIT_CRASH_AFTER_COMMIT_CHILD", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server_committed = false;
    for _ in 0..120 {
        if ok_file.exists() {
            server_committed = true;
            break;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "child exited before server COMMIT OK"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if !server_committed {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("server did not confirm original business COMMIT");
    }
    assert!(arm_file.exists());
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    println!(
        "owned business COMMIT child pid={} exited={status}",
        child.id()
    );
    let source = MySqlDataSource::connect(
        owned_mysql::options(Some(proxy_port))
            .operation_timeout(Duration::from_secs(5))
            .unwrap(),
    )
    .await
    .unwrap();
    let recovered =
        MySqlDataSourceManager::new(source, seata_at::MySqlJdbcProfile::Connector8Default).unwrap();
    let xid = "commit-changed:crash-after-commit";
    assert_eq!(
        recovered.recover_commit_attempt(xid, 42).await.unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::Applied
    );
    let mut admin = owned_mysql::admin().await;
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    let normal: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=42 AND log_status=0",
    )
    .bind(xid)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(normal, 1);
    let witness: u8 = sqlx::query_scalar(
        "SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42",
    )
    .bind(xid.as_bytes())
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(witness, 1);
    let guard: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=42 AND log_status=1",
    )
    .bind(xid)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(guard, 0);
    println!("new process recovery: business=1 normal={normal} witness={witness} guard={guard}");
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL and proxy holding business COMMIT before forwarding"]
async fn armed_crash_before_server_commit_parent_fences() {
    use std::process::{Command, Stdio};
    let proxy_port: u16 = std::env::var("SEATA_MYSQL_COMMIT_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let arm_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_ARM").unwrap());
    let marker = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_OK").unwrap());
    assert!(!arm_file.exists() && !marker.exists());
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("armed_crash_after_commit_child")
        .arg("--exact")
        .arg("--ignored")
        .arg("--test-threads=1")
        .env("SEATA_COMMIT_CRASH_AFTER_COMMIT_CHILD", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut packet_held = false;
    for _ in 0..120 {
        if marker.exists() {
            packet_held = true;
            break;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "child exited before business COMMIT packet"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if !packet_held {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("proxy did not hold original business COMMIT");
    }
    assert_eq!(
        std::fs::read_to_string(&marker).unwrap().trim_end(),
        "original MySQL COMMIT packet received; not forwarded"
    );
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    println!(
        "owned pre-server-COMMIT child pid={} exited={status}",
        child.id()
    );
    let source = MySqlDataSource::connect(
        owned_mysql::options(Some(proxy_port))
            .operation_timeout(Duration::from_secs(5))
            .unwrap(),
    )
    .await
    .unwrap();
    let recovered =
        MySqlDataSourceManager::new(source, seata_at::MySqlJdbcProfile::Connector8Default).unwrap();
    let xid = "commit-changed:crash-after-commit";
    assert_eq!(
        recovered.recover_commit_attempt(xid, 42).await.unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::NotAppliedFenced
    );
    let mut admin = owned_mysql::admin().await;
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    let guard: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=42 AND log_status=1",
    )
    .bind(xid)
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(guard, 1);
    let witness: u8 = sqlx::query_scalar(
        "SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42",
    )
    .bind(xid.as_bytes())
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(witness, 0);
    println!("new process recovery: business=0 guard={guard} witness={witness}");
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_observer_loss_retains_unknown_after_durable_commit() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:observer-loss").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    let closed_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let closed_observer = MySqlDataSourceManager::new(
        closed_source.clone(),
        seata_at::MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    closed_source.close().await.unwrap();
    let error = checked.commit_changed(&closed_observer).await.unwrap_err();
    assert!(error.is_unknown_outcome());
    assert_eq!(
        error.registered_branch().unwrap().identity(),
        receipt.identity()
    );
    assert!(h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CommitChanged,
        seata_db_mysql::MySqlPhaseOneOutcome::ChangedCommitUnknown,
        1,
    );
    assert_eq!(h.closed_counts(&mut socket).await, (1, 1, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn prepared_rollback_reports_failure_only_after_local_rollback() {
    let h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let (prepared, mut socket) = prepared(&h, "prepare:rollback-report").await;
    let peer = tokio::spawn(async move {
        let request = rm_fixture::peer::read(&mut socket).await;
        match request.body {
            Some(Message::BranchReportRequest(report)) => {
                assert_eq!(report.xid.as_deref(), Some("prepare:rollback-report"));
                assert_eq!(report.branch_id, 42);
                assert_eq!(report.status, BranchStatus::PhaseOneFailed);
                assert_eq!(
                    report.application_data.as_deref(),
                    Some("prepare-fixture-app")
                );
            }
            other => panic!("unexpected report: {other:?}"),
        }
        let mut observer = owned_mysql::admin().await;
        assert_eq!(fixture::count(&mut observer, "undo_work").await, 0);
        assert_eq!(fixture::count(&mut observer, "undo_log").await, 0);
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                request.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: rm_fixture::peer::success(),
                }),
            ),
        )
        .await;
        socket
    });
    let receipt = prepared
        .rollback_and_report_failure(&h.rm.client())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.identity().branch_id(), 42);
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::RollbackAndReportFailure,
        seata_db_mysql::MySqlPhaseOneOutcome::FailureReportObserved,
        1,
    );
    let mut socket = peer.await.unwrap();
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn prepared_rollback_report_loss_keeps_rollback_fact_and_receipt() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let (prepared, mut socket) = prepared(&h, "prepare:rollback-report-loss").await;
    let original = prepared.registered_branch().unwrap().clone();
    let peer = tokio::spawn(async move {
        let request = rm_fixture::peer::read(&mut socket).await;
        match request.body {
            Some(Message::BranchReportRequest(report)) => {
                assert_eq!(report.xid.as_deref(), Some("prepare:rollback-report-loss"));
                assert_eq!(report.status, BranchStatus::PhaseOneFailed);
                assert_eq!(report.branch_id, 42);
            }
            other => panic!("unexpected report: {other:?}"),
        }
        let mut observer = owned_mysql::admin().await;
        assert_eq!(fixture::count(&mut observer, "undo_work").await, 0);
        assert_eq!(fixture::count(&mut observer, "undo_log").await, 0);
        drop(socket);
    });
    let error = prepared
        .rollback_and_report_failure(&h.rm.client())
        .await
        .unwrap_err();
    peer.await.unwrap();
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::RollbackAndReportFailure,
        seata_db_mysql::MySqlPhaseOneOutcome::FailureReportUnknown,
        1,
    );
    h.save_phase_metrics("rollback-report-loss.json");
    assert!(error.local_rollback_confirmed());
    assert!(error.is_unknown_outcome());
    assert_eq!(
        error.registered_branch().unwrap().identity(),
        original.identity()
    );
    h.rm.shutdown().await.unwrap();
    h.source.close().await.unwrap();
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn prepared_rollback_driver_loss_does_not_report_failure() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let (prepared, mut socket) = prepared(&h, "prepare:rollback-driver-loss").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let owners: Vec<u64> = sqlx::query_scalar("SELECT DISTINCT THREAD_ID FROM performance_schema.data_locks WHERE OBJECT_SCHEMA=DATABASE() AND OBJECT_NAME='undo_work' AND LOCK_TYPE='TABLE'")
        .fetch_all(&mut h.admin).await.unwrap();
    assert_eq!(owners.len(), 1);
    let connection_id: u64 = sqlx::query_scalar(
        "SELECT PROCESSLIST_ID FROM performance_schema.threads WHERE THREAD_ID=?",
    )
    .bind(owners[0])
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    sqlx::query(&format!("KILL CONNECTION {connection_id}"))
        .execute(&mut h.admin)
        .await
        .unwrap();
    let error = prepared
        .rollback_and_report_failure(&h.rm.client())
        .await
        .unwrap_err();
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::RollbackAndReportFailure,
        seata_db_mysql::MySqlPhaseOneOutcome::FailureRollbackUnconfirmed,
        1,
    );
    h.save_phase_metrics("rollback-driver-loss.json");
    assert!(!error.local_rollback_confirmed());
    assert!(error.is_unknown_outcome());
    assert_eq!(
        error.registered_branch().unwrap().identity(),
        receipt.identity()
    );
    assert!(tokio::time::timeout(
        Duration::from_millis(60),
        rm_fixture::peer::read(&mut socket)
    )
    .await
    .is_err());
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_reports_only_after_durable_witness() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:report-ok").await;
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    let peer = tokio::spawn(async move {
        let request = rm_fixture::peer::read(&mut socket).await;
        match request.body {
            Some(Message::BranchReportRequest(report)) => {
                assert_eq!(report.xid.as_deref(), Some("commit-changed:report-ok"));
                assert_eq!(report.branch_id, 42);
                assert_eq!(report.status, BranchStatus::PhaseOneDone);
                assert_eq!(
                    report.application_data.as_deref(),
                    Some("prepare-fixture-app")
                );
            }
            other => panic!("unexpected report: {other:?}"),
        }
        // 在报告抵达的时刻由独立连接确认本地事务已经持久提交。
        let mut observer = owned_mysql::admin().await;
        assert_eq!(fixture::count(&mut observer, "undo_work").await, 1);
        assert_eq!(fixture::count(&mut observer, "undo_log").await, 1);
        let witness: u8 = sqlx::query_scalar(
            "SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42",
        )
        .bind(b"commit-changed:report-ok".as_slice())
        .fetch_one(&mut observer)
        .await
        .unwrap();
        assert_eq!(witness, 1);
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                request.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: rm_fixture::peer::success(),
                }),
            ),
        )
        .await;
        assert!(tokio::time::timeout(
            Duration::from_millis(60),
            rm_fixture::peer::read(&mut socket)
        )
        .await
        .is_err());
        socket
    });
    let receipt = checked
        .commit_changed_and_report(&h.adapter, &h.rm.client())
        .await
        .unwrap();
    assert_eq!(receipt.identity().branch_id(), 42);
    assert!(h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    let mut socket = peer.await.unwrap();
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CommitChanged,
        seata_db_mysql::MySqlPhaseOneOutcome::ChangedCommitted,
        1,
    );
    h.assert_phase(
        seata_db_mysql::MySqlPhaseOneOperation::CommitAndReport,
        seata_db_mysql::MySqlPhaseOneOutcome::ReportObserved,
        1,
    );
    assert_eq!(h.closed_counts(&mut socket).await, (1, 1, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture and concurrent report callers"]
async fn concurrent_recovered_success_reports_grant_one_durable_sender() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:concurrent-reports").await;
    let receipt = prepared
        .lock_commit_preconditions()
        .await
        .unwrap()
        .commit_changed(&h.adapter)
        .await
        .unwrap();
    let peer = tokio::spawn(async move {
        let request = rm_fixture::peer::read(&mut socket).await;
        match request.body {
            Some(Message::BranchReportRequest(report)) => {
                assert_eq!(
                    report.xid.as_deref(),
                    Some("commit-changed:concurrent-reports")
                );
                assert_eq!(report.branch_id, 42);
                assert_eq!(report.status, BranchStatus::PhaseOneDone);
            }
            other => panic!("unexpected report: {other:?}"),
        }
        rm_fixture::peer::write(
            &mut socket,
            rm_fixture::peer::reply(
                request.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: rm_fixture::peer::success(),
                }),
            ),
        )
        .await;
        assert!(tokio::time::timeout(
            Duration::from_millis(100),
            rm_fixture::peer::read(&mut socket)
        )
        .await
        .is_err());
        socket
    });
    let client_a = h.rm.client();
    let client_b = h.rm.client();
    let second_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let second = MySqlDataSourceManager::new(
        second_source.clone(),
        seata_at::MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    let (a, b) = tokio::join!(
        second.report_recovered_applied(
            &client_a,
            receipt.identity().xid(),
            receipt.identity().branch_id(),
        ),
        h.adapter.report_recovered_applied(
            &client_b,
            receipt.identity().xid(),
            receipt.identity().branch_id(),
        )
    );
    let loser = match (a, b) {
        (Ok(_), Err(error)) | (Err(error), Ok(_)) => error,
        outcomes => panic!("expected exactly one recovered report: {outcomes:?}"),
    };
    assert!(matches!(
        loser,
        seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted
    ));
    let started: u8 = sqlx::query_scalar("SELECT report_started FROM seata_rs_at_commit_attempt")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(started, 1);
    let mut socket = peer.await.unwrap();
    second_source.close().await.unwrap();
    assert_eq!(h.closed_counts(&mut socket).await, (1, 1, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "child process controlled by distinct_processes_compete_for_one_recovered_report"]
async fn recovered_report_race_child() {
    use seata_rm::{ResourceCatalog, ResourceManager};

    if std::env::var("SEATA_REPORT_RACE_CHILD").as_deref() != Ok("1") {
        return;
    }
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = std::sync::Arc::new(
        MySqlDataSourceManager::new(
            source.clone(),
            seata_at::MySqlJdbcProfile::Connector8Default,
        )
        .unwrap(),
    );
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let endpoint = std::env::var("SEATA_REPORT_RACE_RM")
        .unwrap()
        .parse()
        .unwrap();
    let rm = ResourceManager::builder(
        rm_fixture::settings(endpoint),
        rm_fixture::registration(),
        catalog,
        rm_fixture::journal().await,
    )
    .request_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let barrier_endpoint = std::env::var("SEATA_REPORT_RACE_BARRIER").unwrap();
    let mut barrier = TcpStream::connect(barrier_endpoint).await.unwrap();
    let mut signal = [0_u8; 1];
    tokio::time::timeout(
        Duration::from_secs(10),
        tokio::io::AsyncReadExt::read_exact(&mut barrier, &mut signal),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(signal[0], 1);
    let xid = std::env::var("SEATA_REPORT_RACE_XID").unwrap();
    let branch_id = std::env::var("SEATA_REPORT_RACE_BRANCH")
        .unwrap()
        .parse()
        .unwrap();
    match adapter
        .report_recovered_applied(&rm.client(), &xid, branch_id)
        .await
    {
        Ok(()) => println!("REPORT_RACE=sent"),
        Err(seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted) => {
            println!("REPORT_RACE=already_started")
        }
        other => panic!("unexpected cross-process report outcome: {other:?}"),
    }
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL and two independent recovery processes"]
async fn distinct_processes_compete_for_one_recovered_report() {
    use std::process::{Command, Stdio};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let xid = "commit-changed:process-race";
    let (prepared, mut original_socket) = prepared(&h, xid).await;
    let receipt = prepared
        .lock_commit_preconditions()
        .await
        .unwrap()
        .commit_changed(&h.adapter)
        .await
        .unwrap();
    let barrier_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let report_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut children = Vec::new();
    for _ in 0..2 {
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("recovered_report_race_child")
            .arg("--exact")
            .arg("--ignored")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env("SEATA_REPORT_RACE_CHILD", "1")
            .env("SEATA_REPORT_RACE_XID", receipt.identity().xid())
            .env(
                "SEATA_REPORT_RACE_BRANCH",
                receipt.identity().branch_id().to_string(),
            )
            .env(
                "SEATA_REPORT_RACE_RM",
                report_listener.local_addr().unwrap().to_string(),
            )
            .env(
                "SEATA_REPORT_RACE_BARRIER",
                barrier_listener.local_addr().unwrap().to_string(),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        children.push(child);
    }
    // 两个独立进程先全部就位，再释放同一屏障，避免顺序启动掩盖竞争。
    let (mut first, _) = tokio::time::timeout(Duration::from_secs(5), barrier_listener.accept())
        .await
        .unwrap()
        .unwrap();
    let (mut second, _) = tokio::time::timeout(Duration::from_secs(5), barrier_listener.accept())
        .await
        .unwrap()
        .unwrap();
    first.write_all(&[1]).await.unwrap();
    second.write_all(&[1]).await.unwrap();
    let mut first_socket = tokio::time::timeout(
        Duration::from_secs(5),
        callback_peer::registered(&report_listener, h.adapter.key()),
    )
    .await
    .unwrap();
    // 败者也可能先完成 RM 登记，然后发现持久化门闩已被另一进程占用。
    let mut second_socket = tokio::time::timeout(
        Duration::from_secs(1),
        callback_peer::registered(&report_listener, h.adapter.key()),
    )
    .await
    .ok();
    let mut byte = [0_u8; 1];
    let report_socket = if let Some(ref mut second_socket) = second_socket {
        let mut first_byte = [0_u8; 1];
        let mut second_byte = [0_u8; 1];
        let first_ready = first_socket.peek(&mut first_byte);
        let second_ready = second_socket.peek(&mut second_byte);
        tokio::select! {
            result = first_ready => {
                if result.unwrap() == 0 {
                    second_socket
                } else {
                    &mut first_socket
                }
            }
            result = second_ready => {
                if result.unwrap() == 0 {
                    &mut first_socket
                } else {
                    second_socket
                }
            }
        }
    } else {
        &mut first_socket
    };
    let request = rm_fixture::peer::read(report_socket).await;
    match request.body {
        Some(Message::BranchReportRequest(report)) => {
            assert_eq!(report.xid.as_deref(), Some(xid));
            assert_eq!(report.branch_id, receipt.identity().branch_id());
            assert_eq!(report.status, BranchStatus::PhaseOneDone);
        }
        other => panic!("unexpected cross-process report: {other:?}"),
    }
    rm_fixture::peer::write(
        report_socket,
        rm_fixture::peer::reply(
            request.id,
            Message::BranchReportResponse(BranchReportResponse {
                response: rm_fixture::peer::success(),
            }),
        ),
    )
    .await;
    let mut outcomes = Vec::new();
    for child in children {
        let pid = child.id();
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "child {pid}: {stdout}\n{stderr}");
        outcomes.push(stdout);
    }
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| v.contains("REPORT_RACE=sent"))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| v.contains("REPORT_RACE=already_started"))
            .count(),
        1
    );
    println!("distinct recovery processes: {outcomes:?}");
    let started: u8 = sqlx::query_scalar("SELECT report_started FROM seata_rs_at_commit_attempt")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(started, 1);
    assert!(tokio::time::timeout(
        Duration::from_millis(100),
        rm_fixture::peer::read(&mut original_socket)
    )
    .await
    .is_err());
    // 所有已登记连接只允许上面读到的一帧业务报告。
    for socket in [Some(&mut first_socket), second_socket.as_mut()]
        .into_iter()
        .flatten()
    {
        assert!(matches!(
            tokio::time::timeout(Duration::from_millis(100), socket.read(&mut byte)).await,
            Err(_) | Ok(Ok(0))
        ));
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(100), report_listener.accept())
            .await
            .is_err()
    );
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    h.rm.shutdown().await.unwrap();
    h.source.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn experimental_changed_commit_report_loss_preserves_local_success() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:report-loss").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    let peer = tokio::spawn(async move {
        let request = rm_fixture::peer::read(&mut socket).await;
        match request.body {
            Some(Message::BranchReportRequest(report)) => {
                assert_eq!(report.xid.as_deref(), Some("commit-changed:report-loss"));
                assert_eq!(report.branch_id, 42);
                assert_eq!(report.status, BranchStatus::PhaseOneDone);
            }
            other => panic!("unexpected report: {other:?}"),
        }
        let mut observer = owned_mysql::admin().await;
        assert_eq!(fixture::count(&mut observer, "undo_work").await, 1);
        assert_eq!(fixture::count(&mut observer, "undo_log").await, 1);
        let witness: u8 = sqlx::query_scalar(
            "SELECT first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42",
        )
        .bind(b"commit-changed:report-loss".as_slice())
        .fetch_one(&mut observer)
        .await
        .unwrap();
        assert_eq!(witness, 1);
        // 原 TC 可能已收到报告；不发送响应并关闭连接。
        drop(socket);
    });
    let error = checked
        .commit_changed_and_report(&h.adapter, &h.rm.client())
        .await
        .unwrap_err();
    peer.await.unwrap();
    assert!(error.local_commit_confirmed());
    assert!(error.is_unknown_outcome());
    assert_eq!(
        error.registered_branch().unwrap().identity(),
        receipt.identity()
    );
    assert!(h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    let duplicate = h
        .adapter
        .report_recovered_applied(
            &h.rm.client(),
            receipt.identity().xid(),
            receipt.identity().branch_id(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate,
        seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted
    ));
    let report_started: u8 =
        sqlx::query_scalar("SELECT report_started FROM seata_rs_at_commit_attempt")
            .fetch_one(&mut h.admin)
            .await
            .unwrap();
    assert_eq!(report_started, 1);
    h.rm.shutdown().await.unwrap();
    h.source.close().await.unwrap();
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    assert_eq!(recovery::state(&mut h.admin).await, (1, 0, 0));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL and armed response-loss proxy"]
async fn experimental_changed_commit_server_ok_loss_is_unknown_with_durable_witness() {
    let proxy_port: u16 = std::env::var("SEATA_MYSQL_COMMIT_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let arm_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_ARM").unwrap());
    let ok_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_OK").unwrap());
    assert!(!arm_file.exists() && !ok_file.exists());
    let mut h =
        Harness::start_with_source_port(Duration::from_secs(4), Duration::from_secs(2), proxy_port)
            .await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:server-ok-lost").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    sqlx::query("SET GLOBAL log_output='TABLE'")
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=ON")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    std::fs::write(&arm_file, b"armed\n").unwrap();
    let error = tokio::time::timeout(
        Duration::from_millis(5500),
        checked.commit_changed(&h.adapter),
    )
    .await
    .expect("changed COMMIT must finish inside outer watchdog")
    .unwrap_err();
    assert!(matches!(
        &error,
        MySqlChangedCommitError::Unknown {
            source: DbError::Deadline {
                operation: "changed COMMIT"
            },
            ..
        }
    ));
    assert_eq!(
        error.registered_branch().unwrap().identity(),
        receipt.identity()
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !ok_file.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "server COMMIT OK missing"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        std::fs::read_to_string(&ok_file).unwrap().trim_end(),
        "original MySQL COMMIT returned OK; reply held"
    );
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(
        after,
        before + 2,
        "arming and original business COMMIT required"
    );
    assert!(h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    let witness: u8 = sqlx::query_scalar("SELECT first_phase_committed FROM seata_rs_at_branch_v1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(witness, 1);
    assert_eq!(
        h.adapter
            .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
            .await
            .unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::Applied
    );
    println!("changed COMMIT server OK held; one COMMIT; durable witness and Unknown receipt");
    assert_eq!(h.closed_counts(&mut socket).await, (1, 1, (1, 0, 0)));
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL and proxy holding success-report claim COMMIT OK"]
async fn success_report_claim_ok_loss_never_sends_or_retries_original_report() {
    let proxy_port: u16 = std::env::var("SEATA_MYSQL_COMMIT_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let arm_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_ARM").unwrap());
    let ok_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_OK").unwrap());
    assert!(!arm_file.exists() && !ok_file.exists());
    let mut h =
        Harness::start_with_source_port(Duration::from_secs(4), Duration::from_secs(2), proxy_port)
            .await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:report-claim-ok-lost").await;
    let receipt = prepared
        .lock_commit_preconditions()
        .await
        .unwrap()
        .commit_changed(&h.adapter)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL log_output='TABLE'")
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=ON")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    std::fs::write(&arm_file, b"armed\n").unwrap();
    let result = tokio::time::timeout(
        Duration::from_millis(5500),
        h.adapter.report_recovered_applied(
            &h.rm.client(),
            receipt.identity().xid(),
            receipt.identity().branch_id(),
        ),
    )
    .await
    .expect("report claim must finish inside outer watchdog")
    .unwrap_err();
    assert!(matches!(
        result,
        seata_db_mysql::MySqlRecoveredReportError::Claim { .. }
    ));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !ok_file.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "claim COMMIT OK missing"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        std::fs::read_to_string(&ok_file).unwrap().trim_end(),
        "original MySQL COMMIT returned OK; reply held"
    );
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(after, before + 1, "only one report-claim COMMIT allowed");
    let started: u8 = sqlx::query_scalar("SELECT report_started FROM seata_rs_at_commit_attempt")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(started, 1);
    assert!(tokio::time::timeout(
        Duration::from_millis(60),
        rm_fixture::peer::read(&mut socket)
    )
    .await
    .is_err());
    let repeated = h
        .adapter
        .report_recovered_applied(
            &h.rm.client(),
            receipt.identity().xid(),
            receipt.identity().branch_id(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        repeated,
        seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted
    ));
    assert!(tokio::time::timeout(
        Duration::from_millis(60),
        rm_fixture::peer::read(&mut socket)
    )
    .await
    .is_err());
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    h.rm.shutdown().await.unwrap();
    h.source.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL and an intentionally unanswered RM registration"]
async fn cancelled_report_after_durable_claim_does_not_resend() {
    use seata_rm::{ResourceCatalog, ResourceManager};
    use tokio::net::TcpListener;

    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut original_socket) = prepared(&h, "commit-changed:cancel-after-claim").await;
    let receipt = prepared
        .lock_commit_preconditions()
        .await
        .unwrap()
        .commit_changed(&h.adapter)
        .await
        .unwrap();
    let stalled_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(h.adapter.clone()).unwrap();
    let stalled_rm = ResourceManager::builder(
        rm_fixture::settings(stalled_listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        rm_fixture::journal().await,
    )
    .request_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let adapter = h.adapter.clone();
    let client = stalled_rm.client();
    let xid = receipt.identity().xid().to_owned();
    let branch_id = receipt.identity().branch_id();
    let task = tokio::spawn(async move {
        adapter
            .report_recovered_applied(&client, &xid, branch_id)
            .await
    });
    let (mut stalled_socket, _) =
        tokio::time::timeout(Duration::from_secs(3), stalled_listener.accept())
            .await
            .unwrap()
            .unwrap();
    let registration = rm_fixture::peer::read(&mut stalled_socket).await;
    assert!(matches!(
        registration.body,
        Some(Message::RegisterRMRequest(_))
    ));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let started: u8 =
            sqlx::query_scalar("SELECT report_started FROM seata_rs_at_commit_attempt")
                .fetch_one(&mut h.admin)
                .await
                .unwrap();
        if started == 1 {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        !task.is_finished(),
        "report call must still await RM registration"
    );
    assert!(tokio::time::timeout(
        Duration::from_millis(80),
        rm_fixture::peer::read(&mut original_socket)
    )
    .await
    .is_err());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let repeated = h
        .adapter
        .report_recovered_applied(
            &h.rm.client(),
            receipt.identity().xid(),
            receipt.identity().branch_id(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        repeated,
        seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted
    ));
    assert!(tokio::time::timeout(
        Duration::from_millis(80),
        rm_fixture::peer::read(&mut original_socket)
    )
    .await
    .is_err());
    stalled_rm.shutdown().await.unwrap();
    h.rm.shutdown().await.unwrap();
    h.source.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "child process controlled by report_claimed_child_exit_before_send_is_not_replayed"]
async fn report_claimed_child_waits_for_unanswered_rm_registration() {
    use seata_rm::{ResourceCatalog, ResourceManager};

    if std::env::var("SEATA_REPORT_CLAIM_CHILD").as_deref() != Ok("1") {
        return;
    }
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = std::sync::Arc::new(
        MySqlDataSourceManager::new(source, seata_at::MySqlJdbcProfile::Connector8Default).unwrap(),
    );
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let endpoint = std::env::var("SEATA_REPORT_CLAIM_STALLED_RM")
        .unwrap()
        .parse()
        .unwrap();
    let rm = ResourceManager::builder(
        rm_fixture::settings(endpoint),
        rm_fixture::registration(),
        catalog,
        rm_fixture::journal().await,
    )
    .request_timeout(Duration::from_secs(20))
    .build()
    .unwrap();
    let xid = std::env::var("SEATA_REPORT_CLAIM_XID").unwrap();
    let branch_id: i64 = std::env::var("SEATA_REPORT_CLAIM_BRANCH")
        .unwrap()
        .parse()
        .unwrap();
    // 父进程在持久门闩=1且此注册响应尚未返回时终止子进程。
    let _ = adapter
        .report_recovered_applied(&rm.client(), &xid, branch_id)
        .await;
    panic!("stalled RM registration unexpectedly returned");
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL and an isolated report child process"]
async fn report_claimed_child_exit_before_send_is_not_replayed() {
    use std::process::{Command, Stdio};
    use tokio::net::TcpListener;

    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut original_socket) = prepared(&h, "commit-changed:claim-child-exit").await;
    let receipt = prepared
        .lock_commit_preconditions()
        .await
        .unwrap()
        .commit_changed(&h.adapter)
        .await
        .unwrap();
    let stalled_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("report_claimed_child_waits_for_unanswered_rm_registration")
        .arg("--exact")
        .arg("--ignored")
        .arg("--test-threads=1")
        .env("SEATA_REPORT_CLAIM_CHILD", "1")
        .env("SEATA_REPORT_CLAIM_XID", receipt.identity().xid())
        .env(
            "SEATA_REPORT_CLAIM_BRANCH",
            receipt.identity().branch_id().to_string(),
        )
        .env(
            "SEATA_REPORT_CLAIM_STALLED_RM",
            stalled_listener.local_addr().unwrap().to_string(),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (mut stalled_socket, _) =
        tokio::time::timeout(Duration::from_secs(5), stalled_listener.accept())
            .await
            .unwrap()
            .unwrap();
    let registration = rm_fixture::peer::read(&mut stalled_socket).await;
    assert!(matches!(
        registration.body,
        Some(Message::RegisterRMRequest(_))
    ));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let started: u8 =
            sqlx::query_scalar("SELECT report_started FROM seata_rs_at_commit_attempt")
                .fetch_one(&mut h.admin)
                .await
                .unwrap();
        if started == 1 {
            break;
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "report child exited early"
        );
        assert!(
            tokio::time::Instant::now() < deadline,
            "report claim not durable"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(tokio::time::timeout(
        Duration::from_millis(80),
        rm_fixture::peer::read(&mut original_socket)
    )
    .await
    .is_err());
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
    println!("claimed report child pid={} exited={status}", child.id());
    let recovered = MySqlDataSourceManager::new(
        MySqlDataSource::connect(owned_mysql::options(None))
            .await
            .unwrap(),
        seata_at::MySqlJdbcProfile::Connector8Default,
    )
    .unwrap();
    assert_eq!(
        recovered
            .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
            .await
            .unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::Applied
    );
    assert!(matches!(
        recovered
            .report_recovered_applied(
                &h.rm.client(),
                receipt.identity().xid(),
                receipt.identity().branch_id(),
            )
            .await
            .unwrap_err(),
        seata_db_mysql::MySqlRecoveredReportError::AlreadyStarted
    ));
    assert!(tokio::time::timeout(
        Duration::from_millis(80),
        rm_fixture::peer::read(&mut original_socket)
    )
    .await
    .is_err());
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    h.rm.shutdown().await.unwrap();
    h.source.close().await.unwrap();
}

#[cfg(feature = "experimental_changed_commit")]
#[tokio::test]
#[ignore = "requires owned real MySQL and proxy holding the fence COMMIT OK"]
async fn armed_recovery_fence_ok_loss_remains_unknown_until_reobserved() {
    let proxy_port: u16 = std::env::var("SEATA_MYSQL_COMMIT_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let arm_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_ARM").unwrap());
    let ok_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_OK").unwrap());
    assert!(!arm_file.exists() && !ok_file.exists());
    let mut h =
        Harness::start_with_source_port(Duration::from_secs(4), Duration::from_secs(2), proxy_port)
            .await;
    for migration in [
        include_str!("../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql"),
        include_str!("../migrations/006_seata_rs_at_commit_attempt.sql"),
        include_str!("../migrations/007_seata_rs_at_commit_report_started.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (prepared, mut socket) = prepared(&h, "commit-changed:fence-ok-lost").await;
    let receipt = prepared.registered_branch().unwrap().clone();
    let checked = prepared.lock_commit_preconditions().await.unwrap();
    arm_for_recovery_fixture(&mut h.admin, &receipt).await;
    checked.rollback().await.unwrap();
    sqlx::query("SET GLOBAL log_output='TABLE'")
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=ON")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    std::fs::write(&arm_file, b"armed\n").unwrap();
    let error = tokio::time::timeout(
        Duration::from_millis(5500),
        h.adapter
            .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id()),
    )
    .await
    .expect("recovery must finish inside outer watchdog")
    .unwrap_err();
    assert!(error.is_unknown_outcome());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !ok_file.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "fence COMMIT OK missing"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        std::fs::read_to_string(&ok_file).unwrap().trim_end(),
        "original MySQL COMMIT returned OK; reply held"
    );
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(after, before + 1, "only the original fence COMMIT is sent");
    let guard: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=? AND log_status=1",
    )
    .bind(receipt.identity().xid())
    .bind(receipt.identity().branch_id())
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(guard, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(
        h.adapter
            .recover_commit_attempt(receipt.identity().xid(), receipt.identity().branch_id())
            .await
            .unwrap(),
        seata_db_mysql::MySqlCommitRecoveryOutcome::NotAppliedFenced
    );
    println!("fence COMMIT OK held; first recovery Unknown, explicit second recovery fenced");
    assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 0, 0)));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn first_phase_commit_requires_staged_004_witness() {
    for witness_migration in [false, true] {
        let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
        sqlx::raw_sql(include_str!(
            "../migrations/003_seata_rs_at_commit_decision.sql"
        ))
        .execute(&mut h.admin)
        .await
        .unwrap();
        if witness_migration {
            sqlx::raw_sql(include_str!(
                "../migrations/004_seata_rs_at_first_phase_witness.sql"
            ))
            .execute(&mut h.admin)
            .await
            .unwrap();
        }
        let (value, mut socket) = prepared(&h, "commit-check:004-required").await;
        let checked = value.lock_commit_preconditions().await.unwrap();
        let capability = checked.require_first_phase_commit_witness();
        assert_eq!(capability.is_ok(), witness_migration);
        checked.rollback().await.unwrap();
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn no_change_checked_scope_sends_one_real_commit_without_branch_or_undo() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    sqlx::query("SET GLOBAL log_output='TABLE'")
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=ON")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let mut socket = h.accept().await;
    let mut tx = h.source.begin().await.unwrap();
    assert_eq!(
        tx.execute(
            &seata_at::SqlRecognizerFactory::recognize(
                "UPDATE undo_work SET v='absent' WHERE id=99"
            )
            .unwrap(),
            &[]
        )
        .await
        .unwrap(),
        0
    );
    let checked = h
        .adapter
        .prepare_transaction(tx, &h.rm.client(), "commit-no-change:real", None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    assert!(checked.registered_branch().is_none());
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    checked.commit_no_change().await.unwrap();
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(
        after,
        before + 1,
        "the original connection must send COMMIT"
    );
    assert!(tokio::time::timeout(
        Duration::from_millis(60),
        rm_fixture::peer::read(&mut socket)
    )
    .await
    .is_err());
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn changed_registered_scope_cannot_use_no_change_commit() {
    let h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let (value, mut socket) = prepared(&h, "commit-no-change:reject-registered").await;
    let checked = value.lock_commit_preconditions().await.unwrap();
    let error = checked.commit_no_change().await.unwrap_err();
    assert!(!error.is_unknown_outcome());
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn no_change_commit_driver_loss_is_unknown_even_without_a_tc_branch() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    let mut tx = h.source.begin().await.unwrap();
    assert_eq!(
        tx.execute(
            &seata_at::SqlRecognizerFactory::recognize(
                "UPDATE undo_work SET v='absent' WHERE id=99"
            )
            .unwrap(),
            &[]
        )
        .await
        .unwrap(),
        0
    );
    let checked = h
        .adapter
        .prepare_transaction(tx, &h.rm.client(), "commit-no-change:driver-loss", None)
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    let owner: Vec<u64> = sqlx::query_scalar("SELECT DISTINCT THREAD_ID FROM performance_schema.data_locks WHERE OBJECT_SCHEMA=DATABASE() AND OBJECT_NAME='undo_work' AND LOCK_TYPE='TABLE'")
        .fetch_all(&mut h.admin).await.unwrap();
    assert_eq!(
        owner.len(),
        1,
        "one original transaction must hold the table lock"
    );
    let connection_id: u64 = sqlx::query_scalar(
        "SELECT PROCESSLIST_ID FROM performance_schema.threads WHERE THREAD_ID=?",
    )
    .bind(owner[0])
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert!(connection_id > 0);
    sqlx::query(&format!("KILL CONNECTION {connection_id}"))
        .execute(&mut h.admin)
        .await
        .unwrap();
    let error = checked.commit_no_change().await.unwrap_err();
    assert!(error.is_unknown_outcome());
    assert!(error.registered_branch().is_none());
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn late_recovery_definition_changes_cannot_publish_first_phase_witness() {
    let mut observations = Vec::new();
    for (change,expected) in [
        ("ALTER TABLE seata_rs_at_branch_v1 ENGINE=MyISAM","RM recovery transactional base table"),
        ("CREATE TRIGGER late_recovery AFTER UPDATE ON seata_rs_at_branch_v1 FOR EACH ROW INSERT INTO recovery_side_effect VALUES(NEW.branch_id)","RM recovery trigger, foreign key, constraint or partition"),
        ("ALTER TABLE seata_rs_at_branch_v1 DROP PRIMARY KEY","RM recovery sole identity index"),
        ("ALTER TABLE seata_rs_at_branch_v1 MODIFY resource_id VARBINARY(32766) NOT NULL","RM recovery exact column semantics"),
        ("ALTER TABLE seata_rs_at_branch_v1 ADD COLUMN unexpected INT NULL INVISIBLE","RM recovery exact versioned columns"),
        ("ALTER TABLE seata_rs_at_branch_v1 ADD CONSTRAINT late_check CHECK(first_phase_committed IN(0,1))","RM recovery trigger, foreign key, constraint or partition"),
        ("ALTER TABLE seata_rs_at_branch_v1 PARTITION BY HASH(branch_id) PARTITIONS 2","RM recovery trigger, foreign key, constraint or partition"),
    ] {
        let mut h=Harness::start(Duration::from_secs(5),Duration::from_secs(2)).await;
        sqlx::raw_sql(include_str!("../migrations/003_seata_rs_at_commit_decision.sql")).execute(&mut h.admin).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/004_seata_rs_at_first_phase_witness.sql")).execute(&mut h.admin).await.unwrap();
        sqlx::raw_sql("DROP TABLE IF EXISTS recovery_side_effect; CREATE TABLE recovery_side_effect(id BIGINT) ENGINE=MyISAM")
            .execute(&mut h.admin).await.unwrap();
        let (value,mut socket)=prepared(&h,"commit-check:late-recovery").await;
        sqlx::raw_sql(change).execute(&mut h.admin).await.unwrap();
        let result=value.lock_commit_preconditions().await;
        assert!(result.is_err(),"late recovery definition obtained commit arbitration: {change}");
        let error=result.unwrap_err();
        assert_eq!(error.registered_branch().unwrap().identity().branch_id(),42);
        assert_eq!(error.registered_branch().unwrap().identity().xid(),"commit-check:late-recovery");
        assert!(matches!(error.database_error(),DbError::Unsupported{feature} if *feature == expected)
            || matches!(error.database_error(),DbError::Driver{code:Some(1412),..}),
            "wrong rejection for {change}: {error:?}");
        let extra:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM recovery_side_effect),CAST(COALESCE(MAX(first_phase_committed),0) AS SIGNED) FROM seata_rs_at_branch_v1")
            .fetch_one(&mut h.admin).await.unwrap();
        assert_eq!(extra,(0,0));
        let (business,undo,state)=h.closed_counts(&mut socket).await;
        assert_eq!((business,undo,state),(0,0,(1,0,0)));
        observations.push(serde_json::json!({"change":change,"business":business,"undo":undo,"side_effect":extra.0,"first_phase_committed":extra.1,"registered_branch":42,"state":state}));
        println!("late recovery schema rejected; receipt retained; witness and side effects zero: {change}");
    }
    fixture::save(
        "late-recovery-schema.json",
        &serde_json::json!(observations),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn late_valid_witness_migration_cannot_change_prepared_scope_capability() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    sqlx::raw_sql(include_str!(
        "../migrations/003_seata_rs_at_commit_decision.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    let (value, mut socket) = prepared(&h, "commit-check:late-migration").await;
    sqlx::raw_sql(include_str!(
        "../migrations/004_seata_rs_at_first_phase_witness.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    let error = value.lock_commit_preconditions().await.unwrap_err();
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    assert!(
        matches!(
            error.database_error(),
            DbError::Unsupported {
                feature: "RM recovery schema changed after transaction metadata snapshot"
            } | DbError::Driver {
                code: Some(1412),
                ..
            }
        ),
        "{error:?}"
    );
    let witness: i64 = sqlx::query_scalar(
        "SELECT CAST(first_phase_committed AS SIGNED) FROM seata_rs_at_branch_v1",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(witness, 0);
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
    println!("valid late witness migration rejected; receipt retained; witness zero");
}

async fn prepared(h: &Harness, xid: &str) -> (MySqlPreparedTransaction, TcpStream) {
    let client = h.rm.client();
    let transaction = h.transaction().await;
    let server = async {
        let mut socket = h.accept().await;
        let id = first_fixture::request(&mut socket, xid).await;
        first_fixture::positive(&mut socket, id).await;
        socket
    };
    let (value, socket) = tokio::join!(
        h.adapter.prepare_transaction(
            transaction,
            &client,
            xid,
            Some("prepare-fixture-app".into())
        ),
        server
    );
    (value.unwrap(), socket)
}

async fn wait_on(h: &mut Harness, table: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let waits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME=?")
            .bind(table).fetch_one(&mut h.admin).await.unwrap();
        if waits > 0 {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "actual {table} lock wait required"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn changed_identity_direction_origin_or_schema_never_obtains_commit_arbitration() {
    let mutations = [
        ("commit", "UPDATE seata_rs_at_branch_v1 SET phase=1"),
        ("rollback", "UPDATE seata_rs_at_branch_v1 SET phase=2"),
        (
            "terminal",
            "UPDATE seata_rs_at_branch_v1 SET phase=2,terminal=1",
        ),
        (
            "application",
            "UPDATE seata_rs_at_branch_v1 SET application_data='changed'",
        ),
        (
            "resource",
            "UPDATE seata_rs_at_branch_v1 SET resource_id='other'",
        ),
        ("profile", "UPDATE seata_rs_at_branch_v1 SET jdbc_profile=2"),
        (
            "lock",
            "UPDATE seata_rs_at_branch_v1 SET lock_key='undo_work:2'",
        ),
        (
            "callback",
            "UPDATE seata_rs_at_branch_v1 SET registration_origin=2,lock_key=NULL,phase=1",
        ),
        ("missing", "DELETE FROM seata_rs_at_branch_v1"),
        (
            "schema",
            "ALTER TABLE seata_rs_at_branch_v1 ADD unexpected INT NULL",
        ),
    ];
    let mut observations = Vec::new();
    for (name, sql) in mutations {
        let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
        let (value, mut socket) = prepared(&h, "commit-check:reject").await;
        sqlx::query(sql).execute(&mut h.admin).await.unwrap();
        let error = value.lock_commit_preconditions().await.unwrap_err();
        assert_eq!(
            error.registered_branch().unwrap().identity().branch_id(),
            42
        );
        if name == "schema" {
            assert!(
                matches!(
                    error.database_error(),
                    DbError::Unsupported { .. }
                        | DbError::Driver {
                            code: Some(1412),
                            ..
                        }
                ),
                "schema rejection: {error:?}"
            );
        } else {
            assert!(
                matches!(error.database_error(), DbError::RecoveryConflict),
                "{name}: {error:?}"
            );
        }
        let (business, undo, state) = h.closed_counts(&mut socket).await;
        assert_eq!((business, undo), (0, 0));
        observations.push(serde_json::json!({"case":name,"receipt":42,"business":business,"undo":undo,"control":state}));
    }
    fixture::save(
        "commit-check-rejections.json",
        &serde_json::json!(observations),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn actual_recovery_lock_wait_deadline_and_cancellation_consume_original_scope() {
    let mut observations = Vec::new();
    for cancel in [false, true] {
        let mut h = Harness::start(Duration::from_millis(1500), Duration::from_secs(2)).await;
        let (value, mut socket) = prepared(&h, "commit-check:wait").await;
        let mut blocker = owned_mysql::admin().await;
        sqlx::raw_sql("BEGIN").execute(&mut blocker).await.unwrap();
        sqlx::query("SELECT xid FROM seata_rs_at_branch_v1 FOR UPDATE")
            .fetch_all(&mut blocker)
            .await
            .unwrap();
        let start = tokio::time::Instant::now();
        let task = tokio::spawn(value.lock_commit_preconditions());
        wait_on(&mut h, "seata_rs_at_branch_v1").await;
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            let error = task.await.unwrap().unwrap_err();
            assert!(matches!(
                error.database_error(),
                DbError::Deadline {
                    operation: "commit preconditions"
                }
            ));
            assert_eq!(
                error.registered_branch().unwrap().identity().branch_id(),
                42
            );
            assert!(start.elapsed() < Duration::from_secs(2));
        }
        let elapsed = start.elapsed().as_millis();
        sqlx::raw_sql("ROLLBACK")
            .execute(&mut blocker)
            .await
            .unwrap();
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
        observations.push(serde_json::json!({"cancel":cancel,"actual_recovery_wait":true,"elapsed_ms":elapsed,"business":0,"undo":0}));
    }
    fixture::save("commit-check-waits.json", &serde_json::json!(observations));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn checked_drop_and_unpolled_check_drop_roll_back_and_release_all_original_locks() {
    for checked in [false, true] {
        let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
        let (value, mut socket) = prepared(&h, "commit-check:drop").await;
        if checked {
            drop(value.lock_commit_preconditions().await.unwrap());
        } else {
            drop(value.lock_commit_preconditions());
        }
        // 另一事务的写入成功同时证明没有遗留恢复行锁；连接关闭等待所有原连接结束。
        sqlx::query("UPDATE seata_rs_at_branch_v1 SET phase=1")
            .execute(&mut h.admin)
            .await
            .unwrap();
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 1, 0)));
    }
    fixture::save(
        "commit-check-drops.json",
        &serde_json::json!({"checked_drop":true,"unpolled_drop":true,"business":0,"undo":0}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn no_change_checks_do_not_require_recovery_or_undo_tables_or_register_a_branch() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
    sqlx::query(
        "RENAME TABLE undo_log TO unchecked_undo, seata_rs_at_branch_v1 TO unchecked_recovery",
    )
    .execute(&mut h.admin)
    .await
    .unwrap();
    let mut socket = h.accept().await;
    for zero_row in [false, true] {
        let mut tx = h.source.begin().await.unwrap();
        if zero_row {
            tx.execute(
                &seata_at::SqlRecognizerFactory::recognize("DELETE FROM undo_work WHERE id=99")
                    .unwrap(),
                &[],
            )
            .await
            .unwrap();
        }
        let checked = h
            .adapter
            .prepare_transaction(tx, &h.rm.client(), "commit-check:empty", None)
            .await
            .unwrap()
            .lock_commit_preconditions()
            .await
            .unwrap();
        assert!(checked.registered_branch().is_none());
        assert_eq!(checked.undo_log_count(), 0);
        assert_eq!(
            checked.profile(),
            seata_at::MySqlJdbcProfile::Connector8Default
        );
        checked.rollback().await.unwrap();
        assert!(tokio::time::timeout(
            Duration::from_millis(60),
            rm_fixture::peer::read(&mut socket)
        )
        .await
        .is_err());
    }
    sqlx::query(
        "RENAME TABLE unchecked_undo TO undo_log, unchecked_recovery TO seata_rs_at_branch_v1",
    )
    .execute(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
    fixture::save(
        "commit-check-noop.json",
        &serde_json::json!({"empty":true,"zero_row":true,"branch_requests":0,"control_rows":0}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn original_callback_after_local_check_waits_then_finishes_after_local_rollback() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let (value, mut socket) = prepared(&h, "commit-check:local-first").await;
    let checked = value.lock_commit_preconditions().await.unwrap();
    let key = h.adapter.key().clone();
    let callback = tokio::spawn(async move {
        let outcome = callback_peer::callback(
            &mut socket,
            &key,
            "commit-check:local-first",
            42,
            -171,
            Some("prepare-fixture-app"),
            BranchPhase::Rollback,
        )
        .await;
        (socket, outcome)
    });
    wait_on(&mut h, "seata_rs_at_branch_v1").await;
    assert!(!callback.is_finished());
    checked.rollback().await.unwrap();
    let (mut socket, outcome) = callback.await.unwrap();
    assert_eq!(
        outcome,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 2, 1)));
    fixture::save(
        "commit-check-local-first.json",
        &serde_json::json!({"actual_recovery_wait":true,"callback_after_local_rollback":true,"guard":1,"terminal":1}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn callback_bound_before_check_never_allows_check_even_during_undo_lock_cycle() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let (value, mut socket) = prepared(&h, "commit-check:callback-first").await;
    let key = h.adapter.key().clone();
    let callback = tokio::spawn(async move {
        let outcome = callback_peer::callback(
            &mut socket,
            &key,
            "commit-check:callback-first",
            42,
            -172,
            Some("prepare-fixture-app"),
            BranchPhase::Rollback,
        )
        .await;
        (socket, outcome)
    });
    wait_on(&mut h, "undo_log").await;
    assert_eq!(recovery::state(&mut h.admin).await, (1, 2, 0));
    let error = value.lock_commit_preconditions().await.unwrap_err();
    assert!(
        matches!(
            error.database_error(),
            DbError::RecoveryConflict
                | DbError::Driver {
                    code: Some(1213),
                    ..
                }
        ),
        "{error:?}"
    );
    assert_eq!(
        error.registered_branch().unwrap().identity().branch_id(),
        42
    );
    let deadlock_victim = matches!(
        error.database_error(),
        DbError::Driver {
            code: Some(1213),
            ..
        }
    );
    let (mut socket, first) = callback.await.unwrap();
    assert!(
        first == (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
            || first
                == (
                    ResultCode::Success,
                    BranchStatus::PhaseTwoRollbackFailedRetryable
                )
    );
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            h.adapter.key(),
            "commit-check:callback-first",
            42,
            -173,
            Some("prepare-fixture-app"),
            BranchPhase::Rollback
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 2, 1)));
    fixture::save(
        "commit-check-callback-first.json",
        &serde_json::json!({"prior_phase":2,"actual_Undo_wait":true,"local_deadlock_victim":deadlock_victim,"checked":false,"guard":1,"terminal":1}),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn original_transaction_holds_commit_arbitration_until_whole_rollback() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let (value, mut socket) = prepared(&h, "commit-check:held").await;
    let checked = value.lock_commit_preconditions().await.unwrap();
    // 独立引擎观察者证明三组记录锁来自同一 InnoDB 事务及同一实际连接，不靠对象标签推断。
    let owners: Vec<(u64, u64, i64)> = sqlx::query_as("SELECT ENGINE_TRANSACTION_ID,THREAD_ID,COUNT(DISTINCT OBJECT_NAME) FROM performance_schema.data_locks WHERE OBJECT_SCHEMA=DATABASE() AND OBJECT_NAME IN ('undo_work','undo_log','seata_rs_at_branch_v1') AND LOCK_TYPE='RECORD' GROUP BY ENGINE_TRANSACTION_ID,THREAD_ID HAVING COUNT(DISTINCT OBJECT_NAME)=3")
        .fetch_all(&mut h.admin).await.unwrap();
    assert_eq!(
        owners.len(),
        1,
        "one actual original transaction must own all three record-lock groups"
    );
    let (transaction_id, thread_id, tables) = owners[0];
    assert!(transaction_id > 0 && thread_id > 0);
    assert_eq!(tables, 3);
    let physical_connection: u64 = sqlx::query_scalar(
        "SELECT PROCESSLIST_ID FROM performance_schema.threads WHERE THREAD_ID=?",
    )
    .bind(thread_id)
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert!(physical_connection > 0);
    assert_eq!(
        checked.registered_branch().unwrap().identity().branch_id(),
        42
    );
    assert_eq!(checked.undo_log_count(), 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
    let mut competing = owned_mysql::admin().await;
    let changer = tokio::spawn(async move {
        sqlx::query("UPDATE seata_rs_at_branch_v1 SET phase=2 WHERE branch_id=42")
            .execute(&mut competing)
            .await
            .unwrap()
            .rows_affected()
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let waits: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits")
                .fetch_one(&mut h.admin)
                .await
                .unwrap();
        if waits > 0 {
            break;
        }
        assert!(!changer.is_finished());
        assert!(
            tokio::time::Instant::now() < deadline,
            "actual competing lock wait required"
        );
        tokio::task::yield_now().await;
    }
    assert!(!changer.is_finished());
    checked.rollback().await.unwrap();
    assert_eq!(changer.await.unwrap(), 1);
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 2, 0)));
    fixture::save(
        "commit-check-held.json",
        &serde_json::json!({
        "branch_id":42,"actual_recovery_row_wait":true,"same_original_scope":true,
        "actual_engine_transaction_id":transaction_id,"actual_thread_id":thread_id,
        "actual_physical_connection_id":physical_connection,"same_transaction_record_lock_tables":3,
            "business_rows":0,"undo_rows":0,"public_business_COMMIT":false
        }),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn original_eleven_column_and_migrated_version_one_records_keep_commit_checks_compatible() {
    let mut observations = Vec::new();
    for legacy in [true, false] {
        let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
        if legacy {
            sqlx::query("ALTER TABLE seata_rs_at_branch_v1 DROP COLUMN registration_origin")
                .execute(&mut h.admin)
                .await
                .unwrap();
        }
        let (value, mut socket) = prepared(&h, "commit-check:legacy").await;
        if !legacy {
            sqlx::query("UPDATE seata_rs_at_branch_v1 SET schema_version=1")
                .execute(&mut h.admin)
                .await
                .unwrap();
        }
        let version: u16 = sqlx::query_scalar("SELECT schema_version FROM seata_rs_at_branch_v1")
            .fetch_one(&mut h.admin)
            .await
            .unwrap();
        assert_eq!(version, 1);
        let checked = value.lock_commit_preconditions().await.unwrap();
        assert_eq!(
            checked.registered_branch().unwrap().identity().branch_id(),
            42
        );
        checked.rollback().await.unwrap();
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 0, 0)));
        observations.push(serde_json::json!({"legacy_eleven_columns":legacy,"row_version":1,"business":0,"undo":0,"control":[1,0,0]}));
    }
    fixture::save("commit-check-legacy.json", &serde_json::json!(observations));
}
