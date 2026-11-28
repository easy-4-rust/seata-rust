//! 饱和计数在并行归还作用域时仍不回绕，active 最终归零。
#![allow(clippy::unwrap_used)]
use super::RmCallMetricsSampler;
use crate::{RmCallError, RmOperation};
use std::sync::atomic::Ordering;

#[test]
fn concurrent_completions_saturate_without_losing_active_release() {
    let sampler = RmCallMetricsSampler::new();
    sampler.counts[0][1].store(u64::MAX - 2, Ordering::Relaxed);
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let sampler = sampler.clone();
            scope.spawn(move || {
                let guard = sampler.begin(RmOperation::BranchRegister);
                guard.finish(&Ok::<(), RmCallError>(()));
            });
        }
    });
    let metrics = sampler.snapshot(RmOperation::BranchRegister);
    assert_eq!(metrics.observed, u64::MAX);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.cancelled, 0);
}
