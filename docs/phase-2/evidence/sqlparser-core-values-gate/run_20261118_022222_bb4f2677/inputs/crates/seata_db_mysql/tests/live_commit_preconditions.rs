//! 原业务事务中的提交前仲裁；当前仍没有业务 COMMIT 入口。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
#[allow(dead_code)]
mod callback_peer;
#[path = "support/first_phase_fixture.rs"]
mod first_fixture;
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/rm_recovery_fixture.rs"]
#[allow(dead_code)]
mod recovery;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;

use first_fixture::Harness;
use seata_db_mysql::{DbError, MySqlPreparedTransaction};
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchResource};
use std::time::Duration;
use tokio::net::TcpStream;

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
