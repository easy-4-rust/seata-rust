//! 提交授权、旧行升级和只清理原日志的真实数据库边界。
use super::{callback_peer, enrolled, enrolled_at, fixture};
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchResolution, BranchResource};
use seata_rm::{BranchRecordState, FileBranchStore, JournalOptions};
use std::path::PathBuf;
use std::time::Duration;

use super::child_helper::child;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture and archived actual v2 executable"]
async fn actual_v2_reader_refuses_fourteen_columns_and_current_reader_recovers_pending_rollback() {
    let (mut h, receipt, mut socket) = enrolled().await;
    let out = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap())
        .join("commit-migration-children");
    std::fs::create_dir_all(&out).unwrap();
    let journal = out.join("branches.log");
    let open = || {
        FileBranchStore::open(
            journal.clone(),
            JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
        )
    };
    let store = open().await.unwrap();
    store
        .lease(receipt.identity().clone(), BranchPhase::Rollback)
        .await
        .unwrap()
        .mark_pending()
        .await
        .unwrap();
    drop(store);
    let old = child(
        PathBuf::from(std::env::var("SEATA_RM_V2_CHILD_EXE").unwrap()),
        journal.clone(),
        out.join("old.json"),
    )
    .await;
    assert!(!old.status.success());
    assert!(String::from_utf8_lossy(&old.stderr).contains("persisted recovery did not complete"));
    assert_eq!(state(&mut h.admin).await, (3, 0, 0, 0, 0));
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
    let store = open().await.unwrap();
    assert_eq!(
        store
            .lease(receipt.identity().clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    drop(store);
    let current = child(
        PathBuf::from(std::env::var("SEATA_RM_CHILD_EXE").unwrap()),
        journal,
        out.join("current.json"),
    )
    .await;
    assert!(
        current.status.success(),
        "{}",
        String::from_utf8_lossy(&current.stderr)
    );
    assert_eq!(state(&mut h.admin).await, (3, 2, 1, 0, 0));
    assert_eq!(h.closed_counts(&mut socket).await, (0, 1, (1, 2, 1)));
    fixture::save(
        "commit-direction-old-reader.json",
        &serde_json::json!({"old_exit":old.status.code(),"new_exit":current.status.code(),"old_pending_preserved":true,"old_metadata_rejected":true,"row_version":3,"rollback_terminal":1,"commit_authorized":0,"commit_terminal":0}),
    );
}

pub(super) async fn log(
    admin: &mut sqlx::MySqlConnection,
    xid: &str,
    id: i64,
    context: &str,
    status: i32,
) {
    sqlx::query("INSERT INTO undo_log VALUES(?,?,?,?,?,now(6),now(6))")
        .bind(id)
        .bind(xid)
        .bind(context)
        .bind(b"opaque-not-a-codec-document".as_slice())
        .bind(status)
        .execute(admin)
        .await
        .unwrap();
}
pub(super) async fn state(admin: &mut sqlx::MySqlConnection) -> (u16, u8, u8, u8, u8) {
    sqlx::query_as("SELECT schema_version,phase,terminal,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1")
        .fetch_one(admin).await.unwrap()
}
pub(super) async fn wait_table(admin: &mut sqlx::MySqlConnection, table: &str) {
    let until = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME=?")
            .bind(table).fetch_one(&mut *admin).await.unwrap();
        if n > 0 {
            return;
        }
        assert!(
            tokio::time::Instant::now() < until,
            "actual {table} wait required"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn normal_guard_absent_and_many_orphan_children_cleanup_preserves_other_branches() {
    let mut observations = Vec::new();
    for mode in ["normal", "guard", "absent", "many_orphans"] {
        let (mut h, _, mut socket) = enrolled().await;
        sqlx::query("INSERT INTO undo_work VALUES(1,'committed-fixture')")
            .execute(&mut h.admin)
            .await
            .unwrap();
        if mode == "normal" {
            log(
                &mut h.admin,
                "rollback:xid",
                42,
                "serializer=unregistered&compressorType=GZIP",
                0,
            )
            .await;
        }
        if mode == "guard" {
            log(
                &mut h.admin,
                "rollback:xid",
                42,
                "serializer=jackson&compressorType=NONE",
                1,
            )
            .await;
        }
        if mode == "many_orphans" {
            for id in 1000..1130 {
                log(&mut h.admin, "rollback:xid", id, "branchId=42", 0).await;
            }
        }
        for (xid, id, context) in [
            ("rollback:xid", 5000, "serializer=sentinel"),
            ("rollback:xid", 5001, "branchId=5000"),
            ("other:xid", 6000, "branchId=42"),
        ] {
            log(&mut h.admin, xid, id, context, 0).await;
        }
        let before = fixture::count(&mut h.admin, "undo_log").await;
        assert_eq!(
            callback_peer::callback(
                &mut socket,
                h.adapter.key(),
                "rollback:xid",
                42,
                -201,
                None,
                BranchPhase::Commit
            )
            .await,
            (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
        );
        assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 1));
        let remaining: Vec<i64> =
            sqlx::query_scalar("SELECT branch_id FROM undo_log ORDER BY branch_id")
                .fetch_all(&mut h.admin)
                .await
                .unwrap();
        assert_eq!(remaining, [5000, 5001, 6000]);
        assert_eq!(h.closed_counts(&mut socket).await, (1, 3, (1, 1, 0)));
        observations.push(serde_json::json!({"mode":mode,"before":before,"after":3,"remaining":remaining,"business_rows":1,"commit_terminal":1}));
    }
    fixture::save(
        "commit-direction-log-shapes.json",
        &serde_json::json!(observations),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn public_commit_cannot_authorize_or_cleanup_new_physical_records() {
    let mut observations = Vec::new();
    for phase in [0, 1] {
        let (mut h, receipt, mut socket) = enrolled().await;
        sqlx::query("UPDATE seata_rs_at_branch_v1 SET phase=?")
            .bind(phase)
            .execute(&mut h.admin)
            .await
            .unwrap();
        log(&mut h.admin, "rollback:xid", 42, "serializer=opaque", 0).await;
        for recovery in [false, true] {
            let value = if recovery {
                h.adapter
                    .recover(receipt.identity(), BranchPhase::Commit)
                    .await
            } else {
                h.adapter
                    .apply(receipt.identity(), BranchPhase::Commit)
                    .await
            };
            assert_eq!(value.unwrap(), BranchResolution::Unknown);
            assert_eq!(state(&mut h.admin).await, (3, phase, 0, 0, 0));
            assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
        }
        assert_eq!(
            callback_peer::callback(
                &mut socket,
                h.adapter.key(),
                "rollback:xid",
                42,
                -202,
                None,
                BranchPhase::Commit
            )
            .await,
            (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
        );
        assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 1));
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 1, 0)));
        observations.push(serde_json::json!({"initial_phase":phase,"manual_apply":"Unknown","manual_recover":"Unknown","auth_before_callback":0,"auth_after_callback":1,"terminal":1}));
    }
    fixture::save(
        "commit-direction-authority.json",
        &serde_json::json!(observations),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn upgrading_real_legacy_phase_one_rows_preserves_bytes_and_requires_fresh_callback() {
    let mut observations = Vec::new();
    for physical in [1, 2] {
        let (mut h, receipt, mut socket) = enrolled_at(physical, Duration::from_secs(5)).await;
        assert_eq!(
            h.adapter
                .apply(receipt.identity(), BranchPhase::Commit)
                .await
                .unwrap(),
            BranchResolution::Unknown
        );
        log(&mut h.admin, "rollback:xid", 42, "serializer=opaque", 0).await;
        let snapshot="SELECT CONCAT_WS('|',schema_version,HEX(xid),branch_id,HEX(resource_id),IFNULL(HEX(application_data),'NULL'),IFNULL(HEX(lock_key),'NULL'),jdbc_profile,phase,terminal,DATE_FORMAT(created_at,'%Y-%m-%d %H:%i:%s.%f'),DATE_FORMAT(modified_at,'%Y-%m-%d %H:%i:%s.%f')) FROM seata_rs_at_branch_v1";
        let before: String = sqlx::query_scalar(snapshot)
            .fetch_one(&mut h.admin)
            .await
            .unwrap();
        if physical == 1 {
            sqlx::raw_sql(include_str!(
                "../../migrations/002_seata_rs_at_registration_origin.sql"
            ))
            .execute(&mut h.admin)
            .await
            .unwrap();
        }
        sqlx::raw_sql(include_str!(
            "../../migrations/003_seata_rs_at_commit_decision.sql"
        ))
        .execute(&mut h.admin)
        .await
        .unwrap();
        let after: String = sqlx::query_scalar(snapshot)
            .fetch_one(&mut h.admin)
            .await
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(state(&mut h.admin).await, (physical as u16, 1, 0, 0, 0));
        assert_eq!(
            h.adapter
                .recover(receipt.identity(), BranchPhase::Commit)
                .await
                .unwrap(),
            BranchResolution::Unknown
        );
        assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
        assert_eq!(
            callback_peer::callback(
                &mut socket,
                h.adapter.key(),
                "rollback:xid",
                42,
                -203,
                None,
                BranchPhase::Commit
            )
            .await,
            (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
        );
        assert_eq!(state(&mut h.admin).await, (physical as u16, 1, 0, 1, 1));
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 1, 0)));
        observations.push(serde_json::json!({"old_physical":physical,"row_version_preserved":physical,"old_fields_bytes_preserved":true,"auth_after_migration":0,"new_callback_required":true,"terminal":1}));
    }
    fixture::save(
        "commit-direction-legacy.json",
        &serde_json::json!(observations),
    );
}
