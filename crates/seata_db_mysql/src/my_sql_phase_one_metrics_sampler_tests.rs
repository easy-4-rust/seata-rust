use super::MySqlPhaseOneMetricsSampler;
use crate::{
    DbError, MySqlCommitCheckError, MySqlNoChangeCommitError, MySqlPhaseOneOperation as Operation,
    MySqlPhaseOneOutcome as Outcome, MySqlPrepareError,
};
use std::sync::atomic::Ordering;

#[test]
fn phase_one_stage_errors_preserve_registration_and_commit_boundaries() {
    let sampler = MySqlPhaseOneMetricsSampler::new();
    sampler
        .begin(Operation::Prepare)
        .finish_prepare(&Err(MySqlPrepareError::Deadline {
            receipt: None,
            registration_may_have_effect: true,
        }));
    sampler
        .begin(Operation::Prepare)
        .finish_prepare(&Err(MySqlPrepareError::Local {
            source: DbError::Input { reason: "fixture" },
        }));
    sampler
        .begin(Operation::CheckCommit)
        .finish_check(&Err(MySqlCommitCheckError::new(
            None,
            DbError::Deadline {
                operation: "fixture",
            },
        )));
    sampler
        .begin(Operation::CommitNoChange)
        .finish_no_change(&Err(MySqlNoChangeCommitError::Unknown {
            source: DbError::Deadline {
                operation: "fixture",
            },
        }));
    sampler
        .begin(Operation::CommitNoChange)
        .finish_no_change(&Err(MySqlNoChangeCommitError::NotEligible {
            receipt: None,
        }));
    assert_eq!(
        sampler
            .snapshot(Operation::Prepare)
            .count(Outcome::RegistrationUnknown),
        1
    );
    assert_eq!(
        sampler
            .snapshot(Operation::Prepare)
            .count(Outcome::PrepareFailed),
        1
    );
    assert_eq!(
        sampler
            .snapshot(Operation::CheckCommit)
            .count(Outcome::CheckFailed),
        1
    );
    assert_eq!(
        sampler
            .snapshot(Operation::CommitNoChange)
            .count(Outcome::NoChangeCommitUnknown),
        1
    );
    assert_eq!(
        sampler
            .snapshot(Operation::CommitNoChange)
            .count(Outcome::NoChangeNotEligible),
        1
    );
    assert_eq!(
        sampler
            .snapshot(Operation::CommitNoChange)
            .count(Outcome::NoChangeCommitted),
        0
    );
    #[cfg(feature = "experimental_changed_commit")]
    {
        sampler.begin(Operation::CommitChanged).finish_changed(&Err(
            crate::MySqlChangedCommitError::NotEligible {
                receipt: None,
                reason: "fixture",
            },
        ));
        sampler
            .begin(Operation::CommitAndReport)
            .finish_report(&Err(crate::MySqlFirstPhaseReportError::Local(
                crate::MySqlChangedCommitError::NotEligible {
                    receipt: None,
                    reason: "fixture",
                },
            )));
        assert_eq!(
            sampler
                .snapshot(Operation::CommitChanged)
                .count(Outcome::ChangedNotEligible),
            1
        );
        assert_eq!(
            sampler
                .snapshot(Operation::CommitAndReport)
                .count(Outcome::ReportLocalFailed),
            1
        );
        assert_eq!(
            sampler
                .snapshot(Operation::CommitAndReport)
                .count(Outcome::ReportObserved),
            0
        );
    }
    assert!(Operation::ALL
        .iter()
        .all(|op| sampler.snapshot(*op).active == 0));
    assert_eq!(
        Operation::ALL
            .iter()
            .map(|op| 1 + op.outcomes().len())
            .sum::<usize>(),
        63
    );
}

#[test]
fn phase_one_stage_counters_saturate_without_losing_other_stages() {
    let sampler = MySqlPhaseOneMetricsSampler::new();
    sampler.counts[Operation::Prepare as usize][Outcome::PrepareFailed as usize + 1]
        .store(u64::MAX, Ordering::Relaxed);
    sampler
        .begin(Operation::Prepare)
        .finish_prepare(&Err(MySqlPrepareError::Local {
            source: DbError::Input { reason: "fixture" },
        }));
    assert_eq!(
        sampler
            .snapshot(Operation::Prepare)
            .count(Outcome::PrepareFailed),
        u64::MAX
    );
    assert_eq!(sampler.snapshot(Operation::Prepare).active, 0);
    assert_eq!(
        sampler
            .snapshot(Operation::CheckCommit)
            .count(Outcome::PrepareFailed),
        0
    );
}

#[test]
fn phase_one_stage_clones_share_counts_and_cancelled_scope() {
    let sampler = MySqlPhaseOneMetricsSampler::new();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let clone = sampler.clone();
            scope.spawn(move || {
                for _ in 0..100 {
                    clone
                        .begin(Operation::CommitNoChange)
                        .finish_no_change(&Err(MySqlNoChangeCommitError::NotEligible {
                            receipt: None,
                        }));
                }
            });
        }
    });
    drop(sampler.begin(Operation::Prepare));
    assert!(std::panic::catch_unwind(|| {
        let _guard = sampler.begin(Operation::Prepare);
        panic!("owned first-phase metrics unwind");
    })
    .is_err());
    assert_eq!(sampler.snapshot(Operation::Prepare).active, 0);
    assert_eq!(
        sampler
            .snapshot(Operation::Prepare)
            .count(Outcome::Cancelled),
        2
    );
    assert_eq!(
        sampler
            .snapshot(Operation::CommitNoChange)
            .count(Outcome::NoChangeNotEligible),
        800
    );
    assert_eq!(
        MySqlPhaseOneMetricsSampler::new()
            .snapshot(Operation::Prepare)
            .count(Outcome::Cancelled),
        0
    );
}
