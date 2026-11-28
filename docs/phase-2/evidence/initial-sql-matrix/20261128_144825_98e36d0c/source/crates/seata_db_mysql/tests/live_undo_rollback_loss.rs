//! 补偿与原Undo删除的真实控制COMMIT故障；正式API不接收夹具故障信息。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/undo_rollback_fixture.rs"]
mod rollback_fixture;
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{DbError, MySqlDataSource, UndoRollbackError, UndoRollbackObservation};
use std::time::Duration;
#[path = "support/undo_metrics_snapshot.rs"]
mod undo_metrics_snapshot;
use seata_db_mysql::MySqlUndoOperation;

#[tokio::test]
#[ignore = "requires owned MySQL and original packet COMMIT observer"]
async fn uncertain_rollback_commit_keeps_original_identity_and_recovers_without_business_replay() {
    let (mut admin, seed) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    seed.close().await.unwrap();
    let port = std::env::var("SEATA_ROLLBACK_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let ds = MySqlDataSource::connect(
        owned_mysql::options(Some(port))
            .operation_timeout(Duration::from_millis(1500))
            .unwrap(),
    )
    .await
    .unwrap();
    let metrics = ds.undo_metrics_sampler();
    let error = ds
        .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
        .await
        .unwrap_err();
    assert!(
        matches!(error, UndoRollbackError::CommitOutcomeUnknown { .. }),
        "control result wrongly classified: {error:?}"
    );
    assert_eq!(error.identity(), Some(("rollback:xid", 42)));
    assert!(error.is_unknown_outcome());
    assert!(!format!("{error:?} {error}").contains("rollback:xid"));
    let unknown_metrics = metrics.snapshot(MySqlUndoOperation::Rollback);
    assert_eq!(unknown_metrics.active, 0);
    assert_eq!(unknown_metrics.unknown_commit, 1);
    assert_eq!(unknown_metrics.completed, 0);
    assert_eq!(
        unknown_metrics.before_commit_error + unknown_metrics.cancelled,
        0
    );
    assert_eq!(
        unknown_metrics.consumed
            + unknown_metrics.fenced_inserted
            + unknown_metrics.fenced_existing
            + unknown_metrics.observed_restored_rows,
        0
    );
    let mode = std::env::var("SEATA_ROLLBACK_FAULT").unwrap();
    if mode == "delay_ok" {
        assert!(matches!(
            error,
            UndoRollbackError::CommitOutcomeUnknown {
                source: DbError::Deadline { .. },
                ..
            }
        ));
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let business = fixture::count(&mut admin, "undo_work").await;
    let undo = fixture::count(&mut admin, "undo_log").await;
    assert_eq!(business, i64::from(mode == "block_commit"));
    assert_eq!(undo, business);
    let recovered = ds
        .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
        .await
        .unwrap();
    assert_eq!(
        recovered,
        if mode == "block_commit" {
            UndoRollbackObservation::Consumed {
                logs: 1,
                restored_rows: 1,
            }
        } else {
            UndoRollbackObservation::Fenced { inserted: true }
        }
    );
    let final_metrics = metrics.snapshot(MySqlUndoOperation::Rollback);
    assert_eq!(final_metrics.active, 0);
    assert_eq!(final_metrics.unknown_commit, 1);
    assert_eq!(final_metrics.completed, 1);
    assert_eq!(
        final_metrics.before_commit_error + final_metrics.cancelled,
        0
    );
    assert_eq!(final_metrics.consumed, u64::from(mode == "block_commit"));
    assert_eq!(
        final_metrics.observed_restored_rows,
        u64::from(mode == "block_commit")
    );
    assert_eq!(
        final_metrics.fenced_inserted,
        u64::from(mode != "block_commit")
    );
    fixture::save(
        "rollback-independent-metrics.json",
        &serde_json::json!({"unknown":undo_metrics_snapshot::snapshot(unknown_metrics),"after_explicit_observation":undo_metrics_snapshot::snapshot(final_metrics)}),
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    let final_undo = fixture::count(&mut admin, "undo_log").await;
    assert_eq!(final_undo, i64::from(mode != "block_commit"));
    fixture::save(
        "rollback-wire-observation.json",
        &serde_json::json!({"mode":mode,"unknown":true,"original_xid":error.identity().unwrap().0,"original_branch_id":error.identity().unwrap().1,"before_explicit_recovery":{"business":business,"undo":undo},"after_explicit_recovery":{"business":0,"undo":final_undo},"recovered":format!("{recovered:?}"),"error":format!("{error:?}")}),
    );
    ds.close().await.unwrap();
}
