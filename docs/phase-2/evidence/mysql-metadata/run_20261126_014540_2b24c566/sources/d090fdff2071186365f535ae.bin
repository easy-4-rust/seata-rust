//! 原提交清理确认未知与真实Pending子进程恢复；不重放业务。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
#[allow(dead_code)]
mod callback_peer;
#[path = "support/commit_direction_child.rs"]
mod child_helper;
#[path = "support/commit_direction_fixture.rs"]
#[allow(dead_code)]
mod commit_fixture;
#[path = "support/first_phase_fixture.rs"]
#[allow(dead_code)]
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
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchRecordState, BranchResource, FileBranchStore, JournalOptions};
use std::{path::PathBuf, time::Duration};

#[tokio::test]
#[ignore = "requires owned real MySQL with targeted commit cleanup fault observer"]
async fn original_cleanup_failure_retains_pending_and_actual_process_recovers_without_business_replay(
) {
    let mode = std::env::var("SEATA_COMMIT_DIRECTION_FAULT").unwrap();
    let out = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    let journal = out.join("commit-direction-fault.branches.log");
    let options = || JournalOptions::new(16, 1048576, 65536, 16).unwrap();
    let store = FileBranchStore::open(journal.clone(), options())
        .await
        .unwrap();
    let (mut h, receipt, mut socket) =
        commit_fixture::enrolled_with_store(3, Duration::from_millis(1500), Some(store.clone()))
            .await;
    sqlx::query("INSERT INTO undo_work VALUES(1,'committed-fault-fixture')")
        .execute(&mut h.admin)
        .await
        .unwrap();
    for (id, context) in [
        (42, "serializer=opaque&compressorType=GZIP"),
        (99, "serializer=sentinel"),
    ] {
        sqlx::query("INSERT INTO undo_log VALUES(?,'rollback:xid',?,?,0,now(6),now(6))")
            .bind(id as i64)
            .bind(context)
            .bind(b"opaque-body".as_slice())
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
            -301,
            None,
            BranchPhase::Commit
        )
        .await,
        (
            ResultCode::Success,
            BranchStatus::PhaseTwoCommitFailedRetryable
        )
    );
    let before:(u16,u8,u8,u8,u8)=sqlx::query_as("SELECT schema_version,phase,terminal,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1")
        .fetch_one(&mut h.admin).await.unwrap();
    let committed = matches!(mode.as_str(), "drop_ok" | "delay_ok");
    assert_eq!(before, (3, 1, 0, 1, u8::from(committed)));
    let rows_before = fixture::count(&mut h.admin, "undo_log").await;
    assert_eq!(rows_before, if committed { 1 } else { 2 });
    assert_eq!(
        store
            .lease(receipt.identity().clone(), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    let resource_id = h.adapter.key().resource_id().to_owned();
    h.rm.shutdown().await.unwrap();
    rm_fixture::peer::eof(&mut socket).await;
    h.source.close().await.unwrap();
    drop(store);
    std::fs::copy(&journal, out.join("commit-direction-before-child.log")).unwrap();
    let actual = child_helper::child(
        PathBuf::from(std::env::var("SEATA_RM_COMMIT_CHILD_EXE").unwrap()),
        journal.clone(),
        out.join("commit-direction-child.json"),
    )
    .await;
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    let reopened = FileBranchStore::open(journal, options()).await.unwrap();
    assert_eq!(
        reopened
            .lease(receipt.identity().clone(), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    drop(reopened);
    let after:(u16,u8,u8,u8,u8)=sqlx::query_as("SELECT schema_version,phase,terminal,commit_authorized,commit_terminal FROM seata_rs_at_branch_v1")
        .fetch_one(&mut h.admin).await.unwrap();
    assert_eq!(after, (3, 1, 0, 1, 1));
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 1);
    let remaining: Vec<i64> = sqlx::query_scalar("SELECT branch_id FROM undo_log")
        .fetch_all(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(remaining, [99]);
    let business: String = sqlx::query_scalar("SELECT v FROM undo_work WHERE id=1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(business, "committed-fault-fixture");
    if mode == "delay_ok" {
        tokio::time::sleep(Duration::from_millis(2400)).await;
    }
    fixture::save(
        "commit-direction-fault.json",
        &serde_json::json!({"mode":mode,"xid":"rollback:xid","branch_id":42,"resource_id":resource_id,"first_status":"PhaseTwoCommitFailedRetryable","before":before,"undo_rows_before_child":rows_before,"file_before":"Pending","after":after,"file_after":"Completed","remaining_undo":[99],"business":business,"actual_commit_child":serde_json::from_slice::<serde_json::Value>(&std::fs::read(out.join("commit-direction-child.json")).unwrap()).unwrap(),"business_replayed":false,"public_business_COMMIT":false}),
    );
}
