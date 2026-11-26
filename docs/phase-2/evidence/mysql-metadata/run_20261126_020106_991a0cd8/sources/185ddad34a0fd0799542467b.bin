//! 固定 Java TC 下原 AT 分支的脏数据回滚。

use super::{fixture, java_undo_orphan::undo_rows};
use seata_protocol::GlobalStatus;
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, FileBranchStore};
use seata_tm::{GlobalTransaction, TransactionCompletion};
use sqlx::MySqlConnection;
use std::time::Duration;

/// 制造现值与原 Undo 镜像不一致的场景，核对原 TC 结束状态和数据库事实。
pub(super) async fn exercise(
    admin: &mut MySqlConnection,
    global: &mut GlobalTransaction<'_>,
    store: &FileBranchStore,
    identity: &BranchIdentity,
) {
    let before = undo_rows(admin).await;
    let [root] = before.as_slice() else {
        panic!("expected one original Normal Undo record");
    };
    assert_eq!(
        (root.0, root.1.as_str(), root.4),
        (identity.branch_id(), identity.xid(), 0)
    );
    let changed = sqlx::query("UPDATE undo_work SET v='external_change' WHERE id=1")
        .execute(&mut *admin)
        .await
        .unwrap();
    assert_eq!(changed.rows_affected(), 1);

    let completion = global.rollback().await.unwrap();
    let TransactionCompletion::Observed { outcome, .. } = completion else {
        panic!("dirty rollback requires an observed Java TC result");
    };
    assert_eq!(
        outcome.status(),
        GlobalStatus::RollbackFailed,
        "dirty rollback must stop the original TC automatic retry"
    );

    // 观察两个默认回滚调度周期，原始帧还须独立证明没有后续自动回调。
    tokio::time::sleep(Duration::from_millis(2250)).await;
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    let after = undo_rows(admin).await;
    assert_eq!(before, after);
    let current: String = sqlx::query_scalar("SELECT v FROM undo_work WHERE id=1")
        .fetch_one(&mut *admin)
        .await
        .unwrap();
    assert_eq!(current, "external_change");
    let state: (u8, u8, u8, u8) = sqlx::query_as(
        "SELECT phase,terminal,commit_terminal,first_phase_committed
         FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=?",
    )
    .bind(identity.xid().as_bytes())
    .bind(identity.branch_id())
    .fetch_one(&mut *admin)
    .await
    .unwrap();
    assert_eq!(state, (2, 0, 0, 1));
    fixture::save(
        "java-undo-dirty.json",
        &serde_json::json!({
            "xid": identity.xid(), "branch_id": identity.branch_id(),
            "resource_id": identity.resource().resource_id(), "before": before, "after": after,
            "business": current, "branch_state": state, "tm_status": outcome.status().to_java_v1()
        }),
    );
}
