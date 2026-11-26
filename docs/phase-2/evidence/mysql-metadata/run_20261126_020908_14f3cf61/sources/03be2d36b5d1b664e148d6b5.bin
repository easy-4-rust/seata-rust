//! 准备编排的独立登记 COMMIT 未知；真实回执保留，原回调恢复，不重发注册。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
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
use seata_db_mysql::{MySqlPrepareError, MySqlRecoveryError};
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{BranchPhase, BranchResource};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned MySQL with original enrollment COMMIT fault observer"]
async fn enrollment_commit_loss_retains_known_receipt_and_original_callback_can_finish() {
    let mode = std::env::var("SEATA_PREPARATION_FAULT").unwrap();
    let mut h = Harness::start(Duration::from_millis(1500), Duration::from_secs(4)).await;
    let mut socket = h.accept().await;
    let key = h.adapter.key().clone();
    let tx = h.transaction().await;
    let adapter = h.adapter.clone();
    let client = h.rm.client();
    let task = tokio::spawn(async move {
        adapter
            .prepare_transaction(
                tx,
                &client,
                "prepare:enrollment-fault",
                Some("prepare-fixture-app".into()),
            )
            .await
    });
    let wire = first_fixture::request(&mut socket, "prepare:enrollment-fault").await;
    first_fixture::positive(&mut socket, wire).await;
    let error = task.await.unwrap().unwrap_err();
    assert!(
        matches!(
            &error,
            MySqlPrepareError::Deadline {
                receipt: Some(_),
                ..
            } | MySqlPrepareError::Enrollment {
                source: MySqlRecoveryError::CommitOutcomeUnknown { .. },
                ..
            }
        ),
        "{error:?}"
    );
    let known = error.registered_branch().unwrap();
    assert_eq!(known.identity().branch_id(), 42);
    assert_eq!(known.lock_key(), Some("undo_work:1"));
    assert!(!error.is_registration_unknown());
    let before = recovery::state(&mut h.admin).await;
    assert_eq!(
        before,
        if mode == "block_commit" {
            (0, 0, 0)
        } else {
            (1, 0, 0)
        }
    );
    assert_eq!(fixture::count(&mut h.admin, "undo_log").await, 0);
    assert_eq!(fixture::count(&mut h.admin, "undo_work").await, 0);
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "prepare:enrollment-fault",
            42,
            -171,
            Some("prepare-fixture-app"),
            BranchPhase::Rollback
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    let origin: u8 = sqlx::query_scalar("SELECT registration_origin FROM seata_rs_at_branch_v1")
        .fetch_one(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(origin, if mode == "block_commit" { 2 } else { 1 });
    let facts = h.closed_counts(&mut socket).await;
    assert_eq!(facts, (0, 1, (1, 2, 1)));
    fixture::save(
        "prepare-enrollment-fault.json",
        &serde_json::json!({"mode":mode,"xid":"prepare:enrollment-fault","branch_id":42,"resource_id":key.resource_id(),"known_lock_key":"undo_work:1","original_register_requests":1,"first":"PreparationFailedWithKnownReceipt","before":before,"after":facts.2,"origin_after_callback":origin,"business_rows":0,"guard_rows":1,"Normal_Undo_rows":0,"business_COMMIT":false,"callback":"PhaseTwoRollbacked"}),
    );
}
