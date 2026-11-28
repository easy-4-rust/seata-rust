#![allow(clippy::unwrap_used)]
#[path = "../tests/support/mod.rs"]
mod support;
use super::{finish_shutdown, SeataClient};
use seata_rm::{FileBranchStore, ResourceCatalog, RmError};
use seata_tm::TmError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;

#[tokio::test]
async fn tm_failure_still_polls_rm_and_preserves_both_original_errors() {
    let polled = AtomicBool::new(false);
    let error = finish_shutdown(
        async {
            Err(TmError::InvalidConfiguration {
                reason: "private-tm-secret",
            })
        },
        async {
            polled.store(true, Ordering::SeqCst);
            Err(RmError::Unavailable {
                reason: "private-rm-secret",
            })
        },
    )
    .await
    .unwrap_err();
    assert!(polled.load(Ordering::SeqCst));
    assert!(matches!(
        error.tm_error(),
        Some(TmError::InvalidConfiguration {
            reason: "private-tm-secret"
        })
    ));
    assert!(matches!(
        error.rm_error(),
        Some(RmError::Unavailable {
            reason: "private-rm-secret"
        })
    ));
    assert!(!format!("{error} {error:?}").contains("secret"));
}

#[tokio::test]
async fn actual_rm_owner_is_drained_when_subsequent_tm_start_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (path, store) = support::journal().await;
    let error = SeataClient::builder(
        support::connection(listener.local_addr().unwrap()),
        support::identity(),
    )
    .enable_tm()
    .enable_rm(ResourceCatalog::new(4, 4096).unwrap(), store)
    .allow_plaintext()
    .start_with_tm(|_| {
        Err(TmError::InvalidConfiguration {
            reason: "private-startup-secret",
        })
    })
    .await
    .unwrap_err();
    assert!(error.configuration_error().is_none());
    assert!(error.tm_error().is_some());
    assert!(error.rm_error().is_none());
    assert!(!format!("{error} {error:?}").contains("secret"));
    drop(
        FileBranchStore::open(path.clone(), support::options())
            .await
            .unwrap(),
    );
    std::fs::remove_file(path).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(80), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn startup_preserves_tm_failure_when_rm_cleanup_cannot_drain() {
    use seata_protocol::BranchType;
    use seata_rm::{BranchIdentity, BranchPhase};
    use seata_transport::ResourceKey;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (path, store) = support::journal().await;
    let lease = store
        .lease(
            BranchIdentity::new(
                "tc:1".into(),
                1,
                ResourceKey::new(BranchType::At, "mysql://resource".into()).unwrap(),
                None,
            )
            .unwrap(),
            BranchPhase::Commit,
        )
        .await
        .unwrap();
    let error = SeataClient::builder(
        support::connection(listener.local_addr().unwrap()),
        support::identity(),
    )
    .enable_tm()
    .enable_rm(ResourceCatalog::new(4, 4096).unwrap(), store)
    .allow_plaintext()
    .callback_timeout(Duration::from_millis(20))
    .start_with_tm(|_| {
        Err(TmError::InvalidConfiguration {
            reason: "private-startup-secret",
        })
    })
    .await
    .unwrap_err();
    assert!(error.tm_error().is_some() && error.rm_error().is_some());
    assert!(!format!("{error} {error:?}").contains("secret"));
    drop(lease);
    drop(
        FileBranchStore::open(path.clone(), support::options())
            .await
            .unwrap(),
    );
    std::fs::remove_file(path).unwrap();
}
