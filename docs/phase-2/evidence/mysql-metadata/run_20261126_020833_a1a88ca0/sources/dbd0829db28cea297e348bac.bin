//! 原身份别名、非法状态和等待取消的数据库验收。
use super::cases::{log, state, wait_table};
use super::{callback_peer, enrolled, enrolled_at, fixture, owned_mysql};
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchResolution, BranchResource};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn collation_aliases_and_distinct_suffixes_never_delete_other_original_identities() {
    let mut observations = Vec::new();
    for mode in [
        "root_case",
        "root_accent",
        "root_space",
        "child_xid",
        "child_context",
        "root_is_child",
    ] {
        let (mut h, _, mut socket) = enrolled().await;
        let root_xid = match mode {
            "root_case" => "ROLLBACK:XID",
            "root_accent" => "ròllback:xid",
            "root_space" => "rollback:xid ",
            _ => "rollback:xid",
        };
        let context = if mode == "root_is_child" {
            "branchId=99"
        } else {
            "serializer=opaque"
        };
        log(&mut h.admin, root_xid, 42, context, 0).await;
        if mode == "child_xid" {
            log(&mut h.admin, "ROLLBACK:XID", 43, "branchId=42", 0).await;
        }
        if mode == "child_context" {
            log(&mut h.admin, "rollback:xid", 43, "BRANCHID=42", 0).await;
        }
        let before = fixture::count(&mut h.admin, "undo_log").await;
        let candidates: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM undo_log WHERE xid=? AND branch_id=42")
                .bind("rollback:xid")
                .fetch_one(&mut h.admin)
                .await
                .unwrap();
        let collation:String=sqlx::query_scalar("SELECT CAST(COLLATION_NAME AS CHAR CHARACTER SET utf8mb4) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='undo_log' AND COLUMN_NAME='xid'")
            .fetch_one(&mut h.admin).await.unwrap();
        let must_refuse = root_xid == "rollback:xid" || candidates > 0;
        let first = callback_peer::callback(
            &mut socket,
            h.adapter.key(),
            "rollback:xid",
            42,
            -204,
            None,
            BranchPhase::Commit,
        )
        .await;
        assert_eq!(
            first,
            if must_refuse {
                (
                    ResultCode::Success,
                    BranchStatus::PhaseTwoCommitFailedRetryable,
                )
            } else {
                (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
            },
            "{mode}, collation={collation}, SQL candidates={candidates}"
        );
        assert_eq!(
            state(&mut h.admin).await,
            (3, 1, 0, 1, u8::from(!must_refuse))
        );
        assert_eq!(fixture::count(&mut h.admin, "undo_log").await, before);
        if must_refuse {
            // 只在自有夹具显式修复坏日志，验证Pending可重试；产品路径不改写任何身份。
            sqlx::query("UPDATE undo_log SET xid='rollback:xid'")
                .execute(&mut h.admin)
                .await
                .unwrap();
            sqlx::query("UPDATE undo_log SET context='serializer=opaque' WHERE branch_id=42")
                .execute(&mut h.admin)
                .await
                .unwrap();
            sqlx::query("UPDATE undo_log SET context='branchId=42' WHERE branch_id=43")
                .execute(&mut h.admin)
                .await
                .unwrap();
            assert_eq!(
                callback_peer::callback(
                    &mut socket,
                    h.adapter.key(),
                    "rollback:xid",
                    42,
                    -207,
                    None,
                    BranchPhase::Commit
                )
                .await,
                (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
            );
        }
        let expected_after = if must_refuse { 0 } else { before };
        assert_eq!(
            h.closed_counts(&mut socket).await,
            (0, expected_after, (1, 1, 0))
        );
        observations.push(serde_json::json!({"case":mode,"column_collation":collation,"sql_root_candidates":candidates,"before":before,"rows_preserved_before_repair":before,"first_refused":must_refuse,"explicit_fixture_repair_then_retry":must_refuse,"after":expected_after,"commit_terminal_after":1}));
    }
    fixture::save(
        "commit-direction-aliases.json",
        &serde_json::json!(observations),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn illegal_direction_authority_and_dual_terminal_combinations_are_never_applied() {
    let invalid = [
        (0, 0, 1, 0),
        (0, 0, 0, 1),
        (1, 0, 0, 1),
        (1, 1, 1, 1),
        (2, 0, 1, 0),
        (2, 0, 0, 1),
        (1, 0, 2, 0),
        (1, 0, 1, 2),
        (1, 2, 1, 0),
    ];
    let mut observations = Vec::new();
    for (phase, rollback, auth, commit) in invalid {
        let (mut h, receipt, mut socket) = enrolled().await;
        log(&mut h.admin, "rollback:xid", 42, "serializer=opaque", 0).await;
        sqlx::query("UPDATE seata_rs_at_branch_v1 SET phase=?,terminal=?,commit_authorized=?,commit_terminal=?")
            .bind(phase).bind(rollback).bind(auth).bind(commit).execute(&mut h.admin).await.unwrap();
        assert_eq!(
            h.adapter
                .recover(receipt.identity(), BranchPhase::Commit)
                .await
                .unwrap(),
            BranchResolution::Unknown
        );
        let unchanged = state(&mut h.admin).await;
        assert_eq!(unchanged, (3, phase, rollback, auth, commit));
        let counts = h.closed_counts(&mut socket).await;
        assert_eq!((counts.0, counts.1), (0, 1));
        observations
            .push(serde_json::json!({"invalid":unchanged,"result":"Unknown","undo_rows":1}));
    }
    fixture::save(
        "commit-direction-invalid-states.json",
        &serde_json::json!(observations),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn authorized_cleanup_deadline_and_mid_wait_cancellation_preserve_logs_then_original_callback_recovers(
) {
    let mut observations = Vec::new();
    for cancel in [false, true] {
        let (mut h, receipt, mut socket) = enrolled_at(3, Duration::from_millis(900)).await;
        log(&mut h.admin, "rollback:xid", 42, "serializer=opaque", 0).await;
        let mut blocker = owned_mysql::admin().await;
        sqlx::raw_sql("BEGIN").execute(&mut blocker).await.unwrap();
        sqlx::query("SELECT branch_id FROM undo_log WHERE branch_id=42 FOR UPDATE")
            .fetch_all(&mut blocker)
            .await
            .unwrap();
        let key = h.adapter.key().clone();
        let callback = tokio::spawn(async move {
            let value = callback_peer::callback(
                &mut socket,
                &key,
                "rollback:xid",
                42,
                -205,
                None,
                BranchPhase::Commit,
            )
            .await;
            (socket, value)
        });
        wait_table(&mut h.admin, "undo_log").await;
        let (returned, outcome) = callback.await.unwrap();
        socket = returned;
        assert_eq!(
            outcome,
            (
                ResultCode::Success,
                BranchStatus::PhaseTwoCommitFailedRetryable
            )
        );
        assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 0));
        if cancel {
            // 第一次超时连接确实结束后，才验证第二个恢复请求的真实中途取消。
            let until = tokio::time::Instant::now() + Duration::from_secs(2);
            loop {
                let waits: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits")
                        .fetch_one(&mut h.admin)
                        .await
                        .unwrap();
                if waits == 0 {
                    break;
                }
                assert!(tokio::time::Instant::now() < until);
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            let adapter = h.adapter.clone();
            let id = receipt.identity().clone();
            let task = tokio::spawn(async move { adapter.recover(&id, BranchPhase::Commit).await });
            wait_table(&mut h.admin, "undo_log").await;
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        }
        assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
        sqlx::raw_sql("ROLLBACK")
            .execute(&mut blocker)
            .await
            .unwrap();
        assert_eq!(
            callback_peer::callback(
                &mut socket,
                h.adapter.key(),
                "rollback:xid",
                42,
                -206,
                None,
                BranchPhase::Commit
            )
            .await,
            (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
        );
        assert_eq!(state(&mut h.admin).await, (3, 1, 0, 1, 1));
        assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (1, 1, 0)));
        observations.push(serde_json::json!({"cancel_after_actual_wait":cancel,"first_callback":"Retryable","authorization_retained":true,"undo_before_recovery":1,"terminal_after_recovery":1}));
    }
    fixture::save(
        "commit-direction-waits.json",
        &serde_json::json!(observations),
    );
}
