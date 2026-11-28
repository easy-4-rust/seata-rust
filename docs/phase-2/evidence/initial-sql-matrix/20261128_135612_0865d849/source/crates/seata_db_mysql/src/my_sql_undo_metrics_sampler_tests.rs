use crate::{
    DbError, MySqlUndoMetricsSampler, MySqlUndoOperation, UndoFenceError, UndoFenceObservation,
    UndoRollbackError, UndoRollbackObservation,
};

#[test]
fn independent_undo_unknown_results_never_become_success_observations() {
    let sampler = MySqlUndoMetricsSampler::new();
    let guard = sampler.begin(MySqlUndoOperation::Rollback);
    assert_eq!(sampler.snapshot(MySqlUndoOperation::Rollback).active, 1);
    guard.finish_rollback(&Err(UndoRollbackError::CommitOutcomeUnknown {
        xid: "metrics-private-xid".into(),
        branch_id: 42,
        source: DbError::Deadline {
            operation: "fixture",
        },
    }));
    let guard = sampler.begin(MySqlUndoOperation::Fence);
    guard.finish_fence(&Err(UndoFenceError::CommitOutcomeUnknown {
        xid: "metrics-private-xid".into(),
        branch_id: 42,
        source: DbError::Deadline {
            operation: "fixture",
        },
    }));
    for operation in [MySqlUndoOperation::Rollback, MySqlUndoOperation::Fence] {
        let snapshot = sampler.snapshot(operation);
        assert_eq!(snapshot.active, 0);
        assert_eq!(snapshot.unknown_commit, 1);
        assert_eq!(snapshot.completed, 0);
        assert_eq!(snapshot.before_commit_error, 0);
        assert_eq!(snapshot.observed_restored_rows, 0);
        assert_eq!(
            snapshot.fenced_inserted
                + snapshot.fenced_existing
                + snapshot.consumed
                + snapshot.undo_present,
            0
        );
    }
    sampler
        .begin(MySqlUndoOperation::Rollback)
        .finish_rollback(&Err(UndoRollbackError::InvalidIdentity));
    assert_eq!(
        sampler
            .snapshot(MySqlUndoOperation::Rollback)
            .before_commit_error,
        1
    );
}

#[test]
fn independent_undo_observations_and_restored_rows_saturate_without_inventing_effects() {
    let sampler = MySqlUndoMetricsSampler::new();
    for restored_rows in [u64::MAX, 2] {
        sampler
            .begin(MySqlUndoOperation::Rollback)
            .finish_rollback(&Ok(UndoRollbackObservation::Consumed {
                logs: 3,
                restored_rows,
            }));
    }
    sampler
        .begin(MySqlUndoOperation::Rollback)
        .finish_rollback(&Ok(UndoRollbackObservation::Fenced { inserted: false }));
    sampler
        .begin(MySqlUndoOperation::Fence)
        .finish_fence(&Ok(UndoFenceObservation::Fenced { inserted: true }));
    sampler
        .begin(MySqlUndoOperation::Fence)
        .finish_fence(&Ok(UndoFenceObservation::UndoPresent));
    let rollback = sampler.snapshot(MySqlUndoOperation::Rollback);
    assert_eq!(rollback.completed, 3);
    assert_eq!(rollback.consumed, 2);
    assert_eq!(rollback.observed_restored_rows, u64::MAX);
    assert_eq!(rollback.fenced_existing, 1);
    assert_eq!(rollback.fenced_inserted, 0);
    let fence = sampler.snapshot(MySqlUndoOperation::Fence);
    assert_eq!(fence.completed, 2);
    assert_eq!(fence.fenced_inserted, 1);
    assert_eq!(fence.undo_present, 1);
    assert_eq!(fence.observed_restored_rows, 0);
}

#[test]
fn independent_undo_clones_share_concurrent_counts_and_cancelled_guards() {
    let sampler = MySqlUndoMetricsSampler::new();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let clone = sampler.clone();
            scope.spawn(move || {
                for _ in 0..100 {
                    clone
                        .begin(MySqlUndoOperation::Fence)
                        .finish_fence(&Ok(UndoFenceObservation::Fenced { inserted: false }));
                }
            });
        }
    });
    let cancelled = sampler.begin(MySqlUndoOperation::Fence);
    drop(cancelled);
    let unwind = std::panic::catch_unwind(|| {
        let _guard = sampler.begin(MySqlUndoOperation::Fence);
        panic!("owned observation unwind");
    });
    assert!(unwind.is_err());
    let snapshot = sampler.snapshot(MySqlUndoOperation::Fence);
    assert_eq!(snapshot.active, 0);
    assert_eq!(snapshot.completed, 800);
    assert_eq!(snapshot.fenced_existing, 800);
    assert_eq!(snapshot.cancelled, 2);
    assert_eq!(
        MySqlUndoMetricsSampler::new()
            .snapshot(MySqlUndoOperation::Fence)
            .completed,
        0
    );
}
