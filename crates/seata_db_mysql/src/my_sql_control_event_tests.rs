#![allow(clippy::unwrap_used)]
use super::MySqlControlEvent;
use crate::{DbError, MySqlRecoveryError, MySqlRecoveryOperation};
use seata_protocol::BranchType;
use seata_rm::{BranchIdentity, BranchPhase};
use seata_transport::ResourceKey;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
#[path = "../tests/support/java_checksum_events.rs"]
mod capture;
#[test]
fn cancellation_unwind_and_error_logs_keep_only_permitted_correlations() {
    let resource = "jdbc:mysql://user:private-password@127.0.0.1/db?token=private-token";
    let id = BranchIdentity::new(
        "control-events:xid".into(),
        42,
        ResourceKey::new(BranchType::At, resource.into()).unwrap(),
        Some("application-private".into()),
    )
    .unwrap();
    let events = Arc::new(Mutex::new(Vec::<BTreeMap<String, String>>::new()));
    let _guard = tracing::subscriber::set_default(capture::EventCapture(events.clone()));
    assert!(std::panic::catch_unwind(|| {
        let mut event = MySqlControlEvent::new(
            &id,
            MySqlRecoveryOperation::Commit,
            Some(BranchPhase::Commit),
        );
        event.mark_commit_attempted();
        panic!("owned control event unwind");
    })
    .is_err());
    MySqlControlEvent::new(&id, MySqlRecoveryOperation::Enroll, None).finish(&Err(
        MySqlRecoveryError::BeforeCommit {
            identity: Box::new(id.clone()),
            source: DbError::Input {
                reason: "driver-error-private",
            },
        },
    ));
    let values = events.lock().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["outcome"], "cancelled");
    assert_eq!(values[0]["commit_attempted"], "true");
    assert_eq!(values[0]["requested_phase"], "commit");
    assert_eq!(values[1]["outcome"], "before_commit_error");
    assert_eq!(values[1]["commit_attempted"], "false");
    for value in values.iter() {
        assert_eq!(value.len(), 10);
        let text = format!("{value:?}");
        for secret in [
            "private-password",
            "private-token",
            "application-private",
            "driver-error-private",
            "jdbc:mysql",
        ] {
            assert!(!text.contains(secret));
        }
    }
}
