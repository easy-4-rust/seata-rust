//! 原TC提交决定与日志清理；夹具的已提交业务不冒充公开一阶段COMMIT。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
#[allow(dead_code)]
mod callback_peer;
#[path = "support/commit_direction_cases.rs"]
mod cases;
#[path = "support/first_phase_fixture.rs"]
#[allow(dead_code)]
mod first_fixture;
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/commit_direction_guard_cases.rs"]
mod guard_cases;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/rm_recovery_fixture.rs"]
#[allow(dead_code)]
mod recovery;
#[path = "support/commit_direction_recovery_cases.rs"]
mod recovery_cases;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchResource};

#[path = "support/commit_direction_child.rs"]
mod child_helper;
#[path = "support/commit_direction_fixture.rs"]
mod commit_fixture;
use commit_fixture::{enrolled, enrolled_at};

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn real_commit_callback_atomically_cleans_root_children_and_terminal_without_business_changes(
) {
    let (mut h, receipt, mut socket) = enrolled().await;
    sqlx::query("INSERT INTO undo_work VALUES(1,'committed-fixture')")
        .execute(&mut h.admin)
        .await
        .unwrap();
    // 原删除路径不解码正文；未知serializer/压缩及Guard均不触碰业务。
    for (id, context, status) in [
        (42, "serializer=opaque&compressorType=GZIP", 0),
        (43, "branchId=42", 0),
        (44, "branchId=42", 1),
        (99, "serializer=sentinel", 0),
        (100, "branchId=99", 0),
    ] {
        sqlx::query("INSERT INTO undo_log VALUES(?,'rollback:xid',?,?,?,now(6),now(6))")
            .bind(id as i64)
            .bind(context)
            .bind(b"opaque-body".as_slice())
            .bind(status)
            .execute(&mut h.admin)
            .await
            .unwrap();
    }
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            h.adapter.key(),
            "rollback:xid",
            42,
            -181,
            None,
            BranchPhase::Commit
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
    );
    let state: (u8, u8, u8, u8) = sqlx::query_as(
        "SELECT phase,terminal,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    assert_eq!(state, (1, 0, 1, 1));
    let remaining: Vec<i64> =
        sqlx::query_scalar("SELECT branch_id FROM undo_log ORDER BY branch_id")
            .fetch_all(&mut h.admin)
            .await
            .unwrap();
    assert_eq!(remaining, [99, 100]);
    assert_eq!(
        h.adapter
            .recover(receipt.identity(), BranchPhase::Commit)
            .await
            .unwrap(),
        seata_rm::BranchResolution::Applied
    );
    assert!(matches!(
        h.adapter
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await,
        Err(seata_rm::RmError::IdentityConflict)
    ));
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            h.adapter.key(),
            "rollback:xid",
            42,
            -182,
            None,
            BranchPhase::Commit
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoCommitted)
    );
    // 已完成提交后收到迟到回滚，只向原对端报告冲突。
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            h.adapter.key(),
            "rollback:xid",
            42,
            -183,
            None,
            BranchPhase::Rollback
        )
        .await,
        (
            ResultCode::Failed,
            BranchStatus::PhaseTwoRollbackFailedUnretryable
        )
    );
    let value: String = sqlx::query_scalar("SELECT v FROM undo_work WHERE id=1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(value, "committed-fixture");
    assert_eq!(cases::state(&mut h.admin).await, (3, 1, 0, 1, 1));
    assert_eq!(h.closed_counts(&mut socket).await, (1, 2, (1, 1, 0)));
    fixture::save(
        "commit-direction-positive.json",
        &serde_json::json!({"branch_id":42,"callback":"PhaseTwoCommitted","phase":1,"rollback_terminal":0,"commit_authorized":1,"commit_terminal":1,"other_branch_rows":[99,100],"business":"committed-fixture","business_replayed":false,"public_business_COMMIT":false}),
    );
}
