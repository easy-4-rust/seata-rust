use crate::{
    undo_rollback_attempt::UndoRollbackAttempt, DbError, UndoRollbackError, UndoRollbackObservation,
};
use std::{cell::Cell, future::pending};
use tokio::time::{timeout_at, Instant};

const XID: &str = "undo-failure:42";

fn read_error() -> DbError {
    DbError::Driver {
        operation: "Undo locked read",
        sql_state: Some("40001".into()),
        code: Some(1213),
        io_source: None,
    }
}

#[tokio::test]
async fn dirty_failure_survives_explicit_rollback_error() {
    let called = Cell::new(false);
    let mut attempt = UndoRollbackAttempt::default();
    attempt
        .abort(DbError::UndoDirty, async {
            called.set(true);
            Err(sqlx::Error::Protocol("cleanup response lost".into()))
        })
        .await;
    assert!(called.get());
    let result = attempt.finish(XID, 42, Some(Ok(None)), false);
    assert!(matches!(
        result,
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::UndoDirty,
            ..
        })
    ));
    assert_eq!(
        result
            .err()
            .and_then(|error| error.identity().map(|(xid, id)| (xid.to_owned(), id))),
        Some((XID.to_owned(), 42))
    );
}

#[tokio::test]
async fn acquired_read_failure_survives_timeout_while_rolling_back() {
    struct Cleanup<'a>(&'a Cell<bool>);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    let released = Cell::new(false);
    let mut attempt = UndoRollbackAttempt::default();
    let result = timeout_at(Instant::now(), async {
        let guard = Cleanup(&released);
        attempt
            .abort(read_error(), async {
                let _owned_connection = guard;
                pending::<Result<(), sqlx::Error>>().await
            })
            .await;
        Ok(None)
    })
    .await;
    assert!(result.is_err());
    assert!(released.get());
    assert!(matches!(
        attempt.finish(XID, 42, result.ok(), true),
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::Driver { operation: "Undo locked read", code: Some(1213), sql_state: Some(state), .. },
            ..
        }) if state == "40001"
    ));
}

#[test]
fn acquire_failure_and_late_commit_remain_distinguishable() {
    let acquisition = DbError::Driver {
        operation: "Undo rollback acquire",
        sql_state: None,
        code: None,
        io_source: None,
    };
    assert!(matches!(
        UndoRollbackAttempt::default().finish(XID, 42, Some(Err(acquisition)), true),
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::Driver {
                operation: "Undo rollback acquire",
                ..
            },
            ..
        })
    ));
    let mut commit = UndoRollbackAttempt::default();
    commit.start_commit();
    assert!(matches!(
        commit.finish(
            XID,
            42,
            Some(Ok(Some(UndoRollbackObservation::Fenced { inserted: true }))),
            true
        ),
        Err(UndoRollbackError::CommitOutcomeUnknown {
            source: DbError::Deadline { .. },
            ..
        })
    ));
    assert!(matches!(
        UndoRollbackAttempt::default().finish(XID, 42, None, true),
        Err(UndoRollbackError::BeforeCommit {
            source: DbError::Deadline { .. },
            ..
        })
    ));
}
