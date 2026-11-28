//! 并行事件计数在上限处饱和，取消和归还仍维持正确的 active。
use super::RmMetricsSampler;
use crate::{BranchPhase, BranchResolution, RmError};
use std::sync::atomic::Ordering;

#[test]
fn concurrent_completions_saturate_and_release_active() {
    let sampler = RmMetricsSampler::new();
    sampler.counts[0][5].store(u64::MAX - 2, Ordering::Relaxed);
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let sampler = sampler.clone();
            scope.spawn(move || {
                let mut guard = sampler.begin(BranchPhase::Commit);
                guard.resolved(BranchResolution::Applied);
                guard.finish(&Ok::<(), RmError>(()));
            });
        }
    });
    let counts = sampler.snapshot(BranchPhase::Commit);
    assert_eq!(
        (counts.applied, counts.active, counts.cancelled),
        (u64::MAX, 0, 0)
    );
}
