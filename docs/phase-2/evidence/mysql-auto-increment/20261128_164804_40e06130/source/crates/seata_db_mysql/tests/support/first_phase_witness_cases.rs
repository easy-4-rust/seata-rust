//! 一阶段见证的实际 MySQL 可见性及受控回调回归。
use super::{callback_peer, fixture, prepared, wait_on, Harness};
use seata_db_mysql::DbError;
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchResource};
use sqlx::Connection;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn first_phase_witness_is_written_in_the_original_transaction_and_rolls_back_with_it() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
    for migration in [
        include_str!("../../migrations/003_seata_rs_at_commit_decision.sql"),
        include_str!("../../migrations/004_seata_rs_at_first_phase_witness.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    let (value, mut socket) = prepared(&h, "commit-check:witness").await;
    let receipt = value.registered_branch().unwrap().clone();
    let value = value.lock_commit_preconditions().await.unwrap();
    let sql = "SELECT first_phase_committed FROM seata_rs_at_branch_v1";
    let visible: u8 = sqlx::query_scalar(sql)
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(
        visible, 0,
        "uncommitted witness must be invisible to a separate session"
    );
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL READ UNCOMMITTED")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let pending: u8 = sqlx::query_scalar(sql)
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(
        pending, 1,
        "witness must be staged in the original transaction"
    );
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let observer = h.adapter.clone();
    let observed =
        tokio::spawn(async move { observer.first_phase_commit_observed(&receipt).await });
    wait_on(&mut h, "seata_rs_at_branch_v1").await;
    assert!(!observed.is_finished());
    value.rollback().await.unwrap();
    assert!(!observed.await.unwrap().unwrap());
    let rolled_back: u8 = sqlx::query_scalar(sql)
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(rolled_back, 0);
    let (business, undo, state) = h.closed_counts(&mut socket).await;
    assert_eq!((business, undo, state), (0, 0, (1, 0, 0)));
    fixture::save(
        "first-phase-witness-rollback.json",
        &serde_json::json!({
            "visible_before_rollback": visible, "dirty_read_witness": pending,
            "visible_after_rollback": rolled_back, "business_after_rollback": business,
            "undo_after_rollback": undo, "recovery_state": state
            ,"observer_waited_for_actual_record_lock": true
        }),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn first_phase_witness_observation_preserves_legacy_and_survives_real_callback_cleanup() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
    sqlx::raw_sql(include_str!(
        "../../migrations/003_seata_rs_at_commit_decision.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    let xid = "commit-check:observe";
    let (value, mut socket) = prepared(&h, xid).await;
    let receipt = value.registered_branch().unwrap().clone();
    assert!(matches!(
        h.adapter.first_phase_commit_observed(&receipt).await,
        Err(DbError::Unsupported {
            feature: "explicit first-phase witness migration required"
        })
    ));
    fixture::dirty(&mut h.admin).await;
    let (context, payload): (String, Vec<u8>) =
        sqlx::query_as("SELECT context,rollback_info FROM undo_log WHERE xid=? AND branch_id=42")
            .bind(xid)
            .fetch_one(&mut h.admin)
            .await
            .unwrap();
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut h.admin)
        .await
        .unwrap();
    value.rollback().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/004_seata_rs_at_first_phase_witness.sql"
    ))
    .execute(&mut h.admin)
    .await
    .unwrap();
    assert!(!h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());

    // 已提交正样本由同一测试管理事务写入，公开业务 COMMIT 仍是后续门槛。
    let mut tx = h.admin.begin().await.unwrap();
    sqlx::query("INSERT INTO undo_work VALUES(1,'prepared_fixture')")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO undo_log VALUES(42,?,?,?,0,now(6),now(6))")
        .bind(xid)
        .bind(&context)
        .bind(&payload)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE seata_rs_at_branch_v1 SET first_phase_committed=1 WHERE xid=? AND branch_id=42",
    )
    .bind(xid.as_bytes())
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 1);
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);

    // 见证位不能掩盖仍待二阶段处理的原 Normal Undo 缺失。
    sqlx::query("DELETE FROM undo_log WHERE xid=? AND branch_id=42")
        .bind(xid)
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(matches!(
        h.adapter.first_phase_commit_observed(&receipt).await,
        Err(DbError::RecoveryConflict)
    ));
    sqlx::query("INSERT INTO undo_log VALUES(42,?,?,?,0,now(6),now(6))")
        .bind(xid)
        .bind(&context)
        .bind(&payload)
        .execute(&mut h.admin)
        .await
        .unwrap();

    // 完整行与分片存在也不足以证明正文可解码或属于原分支。
    sqlx::query("UPDATE undo_log SET rollback_info=? WHERE xid=? AND branch_id=42")
        .bind(b"not-json".as_slice())
        .bind(xid)
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(matches!(
        h.adapter.first_phase_commit_observed(&receipt).await,
        Err(DbError::UndoCodec(_))
    ));
    let mut wrong_identity: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    wrong_identity["branchId"] = serde_json::json!(43);
    sqlx::query("UPDATE undo_log SET rollback_info=? WHERE xid=? AND branch_id=42")
        .bind(serde_json::to_vec(&wrong_identity).unwrap())
        .bind(xid)
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(matches!(
        h.adapter.first_phase_commit_observed(&receipt).await,
        Err(DbError::RecoveryConflict)
    ));
    sqlx::query("UPDATE undo_log SET rollback_info=? WHERE xid=? AND branch_id=42")
        .bind(&payload)
        .bind(xid)
        .execute(&mut h.admin)
        .await
        .unwrap();

    sqlx::query("UPDATE seata_rs_at_branch_v1 SET lock_key='other'")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(matches!(
        h.adapter.first_phase_commit_observed(&receipt).await,
        Err(DbError::RecoveryConflict)
    ));
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET lock_key='undo_work:1'")
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET first_phase_committed=2")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert!(matches!(
        h.adapter.first_phase_commit_observed(&receipt).await,
        Err(DbError::Unsupported { .. })
    ));
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET first_phase_committed=1")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            h.adapter.key(),
            xid,
            42,
            -414,
            Some("prepare-fixture-app"),
            BranchPhase::Commit
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
    );
    assert!(h
        .adapter
        .first_phase_commit_observed(&receipt)
        .await
        .unwrap());
    let flags: (u16,u8,u8,u8) = sqlx::query_as(
        "SELECT schema_version,phase,commit_terminal,first_phase_committed FROM seata_rs_at_branch_v1")
        .fetch_one(&mut h.admin).await.unwrap();
    assert_eq!(flags, (3, 1, 1, 1));
    let (business, undo, state) = h.closed_counts(&mut socket).await;
    assert_eq!((business, undo, state), (1, 0, (1, 1, 0)));
    fixture::save(
        "first-phase-witness-observed.json",
        &serde_json::json!({
            "legacy_observation":"unsupported","migrated_legacy_witness":0,
            "committed_positive_sample":"owned_fixture_admin_transaction",
            "wrong_lock_rejected":true,"invalid_value_rejected":true,
            "business_after_cleanup":business,"undo_after_cleanup":undo,"persisted_flags":flags
        }),
    );
}
