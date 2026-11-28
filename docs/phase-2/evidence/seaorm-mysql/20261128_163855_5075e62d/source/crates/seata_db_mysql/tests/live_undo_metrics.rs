//! 独立 Undo API 的真实结果、克隆归属和数据源关闭后的只读计数。
#![allow(clippy::unwrap_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/undo_rollback_fixture.rs"]
mod rollback_fixture;
#[path = "support/undo_metrics_snapshot.rs"]
mod undo_metrics_snapshot;
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{
    MySqlDataSource, MySqlUndoOperation, UndoFenceObservation, UndoRollbackObservation,
};

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn independent_undo_metrics_count_observations_without_recounting_database_effects() {
    let (mut admin, source) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    let clone = source.clone();
    let metrics = source.undo_metrics_sampler();
    assert_eq!(
        clone
            .undo_metrics_sampler()
            .snapshot(MySqlUndoOperation::Rollback)
            .completed,
        0
    );
    let never_polled = source.fence_undo_log("not-polled-metrics-xid", 9898);
    drop(never_polled);
    assert_eq!(
        metrics.snapshot(MySqlUndoOperation::Fence),
        Default::default()
    );
    assert_eq!(
        source.fence_undo_log("rollback:xid", 42).await.unwrap(),
        UndoFenceObservation::UndoPresent
    );
    assert_eq!(
        clone
            .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 1,
            restored_rows: 1
        }
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(
        source.fence_undo_log("rollback:xid", 42).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: true }
    );
    assert_eq!(
        clone.fence_undo_log("rollback:xid", 42).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: false }
    );
    assert_eq!(
        source
            .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Fenced { inserted: false }
    );
    assert!(source.fence_undo_log("", 0).await.is_err());
    assert!(clone
        .rollback_undo_log("", 0, MySqlJdbcProfile::Connector8Default)
        .await
        .is_err());
    let fence = metrics.snapshot(MySqlUndoOperation::Fence);
    assert_eq!(fence.active, 0);
    assert_eq!(fence.completed, 3);
    assert_eq!(fence.before_commit_error, 1);
    assert_eq!(fence.undo_present, 1);
    assert_eq!(fence.fenced_inserted, 1);
    assert_eq!(fence.fenced_existing, 1);
    assert_eq!(
        fence.unknown_commit + fence.cancelled + fence.consumed + fence.observed_restored_rows,
        0
    );
    let rollback = metrics.snapshot(MySqlUndoOperation::Rollback);
    assert_eq!(rollback.active, 0);
    assert_eq!(rollback.completed, 2);
    assert_eq!(rollback.before_commit_error, 1);
    assert_eq!(rollback.consumed, 1);
    assert_eq!(rollback.observed_restored_rows, 1);
    assert_eq!(rollback.fenced_existing, 1);
    assert_eq!(
        rollback.unknown_commit + rollback.cancelled + rollback.fenced_inserted,
        0
    );
    let separate = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    assert_eq!(
        separate
            .undo_metrics_sampler()
            .snapshot(MySqlUndoOperation::Fence)
            .completed,
        0
    );
    separate.close().await.unwrap();
    source.close().await.unwrap();
    drop(source);
    drop(clone);
    assert_eq!(metrics.snapshot(MySqlUndoOperation::Rollback), rollback);
    assert_eq!(metrics.snapshot(MySqlUndoOperation::Fence), fence);
    fixture::save(
        "independent-undo-observations.json",
        &serde_json::json!({
        "fence":undo_metrics_snapshot::snapshot(fence),"rollback":undo_metrics_snapshot::snapshot(rollback),
        "business_rows":fixture::count(&mut admin,"undo_work").await,
        "undo_rows":fixture::count(&mut admin,"undo_log").await}),
    );
}
