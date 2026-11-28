#![allow(clippy::unwrap_used)]
use super::MySqlUndoEvent;
use crate::test_event_capture as capture;
use crate::{DbError, UndoFenceError, UndoRollbackError};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[test]
fn invalid_identity_is_not_logged_and_missing_resource_is_explicit() {
    let events = Arc::new(Mutex::new(Vec::<BTreeMap<String, String>>::new()));
    let _capture = tracing::subscriber::set_default(capture::EventCapture(events.clone()));
    for (xid, branch) in [
        ("", 42),
        ("valid", 0),
        ("valid", -1),
        ("x".repeat(129).as_str(), 42),
    ] {
        assert!(MySqlUndoEvent::new(xid, branch, None, "fence").is_none());
    }
    assert!(events.lock().unwrap().is_empty());
    MySqlUndoEvent::new("valid", 42, None, "fence")
        .unwrap()
        .finish_fence(&Err(UndoFenceError::InvalidIdentity));
    let values = events.lock().unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0]["resource_sha256"], "");
    assert_eq!(values[0]["commit_attempted"], "false");
}
#[test]
fn unwind_and_original_errors_keep_only_identity_and_digest() {
    let events = Arc::new(Mutex::new(Vec::<BTreeMap<String, String>>::new()));
    let _capture = tracing::subscriber::set_default(capture::EventCapture(events.clone()));
    let resource = "jdbc:mysql://private-user:private-password@localhost/db?token=private-token";
    assert!(std::panic::catch_unwind(|| {
        let mut e = MySqlUndoEvent::new("valid", 42, Some(resource), "rollback").unwrap();
        e.mark_commit_attempted();
        panic!("owned Undo unwind");
    })
    .is_err());
    MySqlUndoEvent::new("valid", 42, Some(resource), "rollback")
        .unwrap()
        .finish_rollback(&Err(UndoRollbackError::BeforeCommit {
            xid: "valid".into(),
            branch_id: 42,
            source: DbError::Input {
                reason: "private-error",
            },
        }));
    let values = events.lock().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["outcome"], "cancelled");
    assert_eq!(values[0]["commit_attempted"], "true");
    assert_eq!(values[1]["outcome"], "before_commit_error");
    assert_eq!(values[1]["commit_attempted"], "false");
    for value in values.iter() {
        assert_eq!(value.len(), 9);
        assert_eq!(
            value["resource_sha256"],
            format!("{:x}", Sha256::digest(resource.as_bytes()))
        );
        for secret in [
            "private-user",
            "private-password",
            "private-token",
            "private-error",
            "jdbc:mysql",
        ] {
            assert!(!format!("{value:?}").contains(secret));
        }
    }
}
