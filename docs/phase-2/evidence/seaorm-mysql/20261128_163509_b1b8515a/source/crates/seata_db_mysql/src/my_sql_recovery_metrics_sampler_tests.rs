#![allow(clippy::unwrap_used)]
use super::*;
#[test]
fn saturation_and_cancelled_guard_are_bounded_and_separate() {
    let sampler = MySqlRecoveryMetricsSampler::new();
    sampler.counts[0][1].store(u64::MAX - 1, Ordering::Relaxed);
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let sampler = sampler.clone();
            scope.spawn(move || {
                let guard = sampler.begin(MySqlRecoveryOperation::Enroll);
                guard.finish(&Ok(seata_rm::BranchResolution::Unknown));
            });
        }
    });
    let count = sampler.snapshot(MySqlRecoveryOperation::Enroll);
    assert_eq!(
        (
            count.active,
            count.completed,
            count.unresolved,
            count.cancelled
        ),
        (0, u64::MAX, 16, 0)
    );
    drop(sampler.begin(MySqlRecoveryOperation::Rollback));
    assert_eq!(
        sampler.snapshot(MySqlRecoveryOperation::Rollback).cancelled,
        1
    );
}

#[test]
fn unknown_commit_is_not_completed_or_before_commit_error() {
    use crate::{DbError, MySqlRecoveryError};
    use seata_protocol::BranchType;
    use seata_rm::BranchIdentity;
    use seata_transport::ResourceKey;
    let source = MySqlRecoveryMetricsSampler::new();
    let identity = BranchIdentity::new(
        "tc:private".into(),
        42,
        ResourceKey::new(BranchType::At, "private-resource".into()).unwrap(),
        None,
    )
    .unwrap();
    source.begin(MySqlRecoveryOperation::Commit).finish(&Err(
        MySqlRecoveryError::CommitOutcomeUnknown {
            identity: Box::new(identity),
            source: DbError::Deadline {
                operation: "test control",
            },
        },
    ));
    let c = source.snapshot(MySqlRecoveryOperation::Commit);
    assert_eq!(
        (
            c.active,
            c.completed,
            c.unknown_commit,
            c.before_commit_error,
            c.cancelled,
            c.applied,
            c.unresolved
        ),
        (0, 0, 1, 0, 0, 0, 0)
    );
}
