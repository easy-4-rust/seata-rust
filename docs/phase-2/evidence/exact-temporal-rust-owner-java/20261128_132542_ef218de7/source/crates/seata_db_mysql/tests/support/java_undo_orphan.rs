//! 固定 Java TC 对已提交 AT 分支的孤儿 Undo 回调和重试。

use super::fixture;
use seata_protocol::GlobalStatus;
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, FileBranchStore};
use seata_tm::{GlobalTransaction, TransactionCompletion, TransactionManager};
use sqlx::{Connection, MySqlConnection};
use std::time::Duration;
use tokio::time::Instant;

pub(super) type UndoRow = (i64, String, String, Vec<u8>, i32, String, String);

pub(super) async fn undo_rows(admin: &mut MySqlConnection) -> Vec<UndoRow> {
    sqlx::query_as(
        "SELECT branch_id,xid,context,rollback_info,log_status,
                CAST(log_created AS CHAR),CAST(log_modified AS CHAR)
         FROM undo_log ORDER BY branch_id",
    )
    .fetch_all(admin)
    .await
    .unwrap()
}

/// 在自有数据库制造残缺 Undo，验证同一 TC 分支在恢复原根行之前留在 Pending。
pub(super) async fn exercise(
    admin: &mut MySqlConnection,
    global: &mut GlobalTransaction<'_>,
    tm: &TransactionManager,
    store: &FileBranchStore,
    identity: &BranchIdentity,
    existing_guard: bool,
) {
    let xid = identity.xid();
    let branch = identity.branch_id();
    let orphan_id = i64::MAX;
    assert_ne!(branch, orphan_id);
    let original = undo_rows(admin).await;
    assert_eq!(original.len(), 1);
    let (root_id, root_xid, root_context, _, status, _, _) = &original[0];
    assert_eq!((*root_id, root_xid.as_str(), *status), (branch, xid, 0));

    // 只移动根记录的身份和上下文，保留生产一阶段生成的正文和时间戳。
    let mut corrupt = admin.begin().await.unwrap();
    let moved =
        sqlx::query("UPDATE undo_log SET branch_id=?,context=? WHERE xid=? AND branch_id=?")
            .bind(orphan_id)
            .bind(format!("branchId={branch}"))
            .bind(xid)
            .bind(branch)
            .execute(&mut *corrupt)
            .await
            .unwrap();
    assert_eq!(moved.rows_affected(), 1);
    if existing_guard {
        sqlx::query("INSERT INTO undo_log VALUES(?,?,'serializer=jackson&compressorType=NONE',X'7B7D',1,NOW(6),NOW(6))")
            .bind(branch)
            .bind(xid)
            .execute(&mut *corrupt)
            .await
            .unwrap();
    }
    corrupt.commit().await.unwrap();
    let before = undo_rows(admin).await;
    let outcome = global.rollback().await.unwrap();
    assert!(
        matches!(
            &outcome,
            TransactionCompletion::Observed { outcome, .. }
                if outcome.status() == GlobalStatus::RollbackRetrying
        ),
        "orphan Undo must keep original TC in RollbackRetrying: {outcome:?}"
    );
    let after = undo_rows(admin).await;
    assert_eq!(before, after);
    assert_eq!(fixture::count(admin, "undo_work").await, 1);
    let state: (u8, u8, u8) = sqlx::query_as(
        "SELECT phase,terminal,first_phase_committed FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?",
    )
    .bind(xid.as_bytes())
    .bind(branch)
    .fetch_one(&mut *admin)
    .await
    .unwrap();
    assert_eq!(state, (2, 0, 1));
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );

    // 按回调相同的根先、子片后的顺序锁定，在一个事务内恢复原根记录。
    let mut repair = admin.begin().await.unwrap();
    let removed = sqlx::query("DELETE FROM undo_log WHERE xid=? AND branch_id=? AND log_status=1")
        .bind(xid)
        .bind(branch)
        .execute(&mut *repair)
        .await
        .unwrap();
    assert_eq!(removed.rows_affected(), u64::from(existing_guard));
    let restored =
        sqlx::query("UPDATE undo_log SET branch_id=?,context=? WHERE xid=? AND branch_id=?")
            .bind(branch)
            .bind(root_context)
            .bind(xid)
            .bind(orphan_id)
            .execute(&mut *repair)
            .await
            .unwrap();
    assert_eq!(restored.rows_affected(), 1);
    repair.commit().await.unwrap();

    let xid_value = seata_tm::Xid::parse(xid.into()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if tm.get_status(&xid_value).await.unwrap().status() == GlobalStatus::Finished {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "original TC did not finish the repaired branch"
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
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
        "java-undo-orphan.json",
        &serde_json::json!({
            "xid": xid, "branch_id": branch, "resource_id": identity.resource().resource_id(),
            "existing_guard": existing_guard, "before": before, "after": after,
            "first_recovery_state": state, "first_journal_state": "Pending",
            "final_journal_state": "Completed"
        }),
    );
}
