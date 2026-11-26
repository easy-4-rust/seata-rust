//! 持久 TM 来源与发送门禁的真实 TCP 回归。
#![allow(clippy::unwrap_used)]
use seata_tm::{DurableTmError, LauncherOrigin, LauncherStore, TmError, TmOperation};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;

#[path = "support/tm_peer.rs"]
mod fixture;
use fixture::{begin, config, manager, peer, registered};

struct UnexpectedRecoveryGate;

impl seata_tm::RecoveredEndGate for UnexpectedRecoveryGate {
    async fn arm(
        self,
        _manager: &seata_tm::TransactionManager,
    ) -> Result<(seata_tm::Xid, TmOperation), DurableTmError> {
        panic!("a foreign recovery authority must be rejected before touching the gate");
    }
}

#[tokio::test]
async fn recovered_end_authority_is_bound_to_its_manager() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let tm = manager(endpoint, Duration::from_millis(100));
    let (other, foreign_authority) =
        seata_tm::TransactionManager::builder(fixture::settings(endpoint), fixture::identity())
            .build_with_recovered_end_authority()
            .unwrap();
    assert!(matches!(
        tm.send_recovered_end(&foreign_authority, UnexpectedRecoveryGate)
            .await,
        Err(DurableTmError::Tm(TmError::IllegalState { .. }))
    ));
    tm.shutdown().await.unwrap();
    other.shutdown().await.unwrap();
}

#[derive(Default)]
struct RecordingStore {
    events: Arc<Mutex<Vec<&'static str>>>,
    fail_begin: bool,
    fail_send: bool,
}
impl LauncherStore for RecordingStore {
    async fn record_begin(
        &self,
        origin: &LauncherOrigin,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        assert_eq!(origin.xid().as_str(), "tc:durable");
        assert_eq!(origin.business_intent(), "order-1");
        self.events.lock().unwrap().push("begin");
        if self.fail_begin {
            return Err("uncertain source".into());
        }
        Ok(())
    }
    async fn record_end_intent(
        &self,
        _origin: &LauncherOrigin,
        _operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.events.lock().unwrap().push("intent");
        Ok(())
    }
    async fn mark_send_started(
        &self,
        _origin: &LauncherOrigin,
        _operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.events.lock().unwrap().push("send");
        if self.fail_send {
            return Err("uncertain marker".into());
        }
        Ok(())
    }
}

#[tokio::test]
async fn failed_send_marker_never_emits_commit_frame() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:durable")).await;
        let observation =
            tokio::time::timeout(Duration::from_millis(120), peer::read(&mut socket)).await;
        assert!(observation.is_err(), "end frame must not be sent");
    });
    let store = RecordingStore {
        fail_send: true,
        ..Default::default()
    };
    let tx = tm
        .begin_durable(&config(), "order-1", &store)
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    assert_eq!(*store.events.lock().unwrap(), ["begin", "intent", "send"]);
    server.await.unwrap();
    tm.shutdown().await.unwrap();
}

#[tokio::test]
async fn lost_commit_response_cannot_recover_through_old_memory_authority() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_millis(100));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:durable")).await;
        let request = peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(seata_protocol::Message::GlobalCommitRequest(_))
        ));
        drop(socket);
    });
    let store = RecordingStore::default();
    let tx = tm
        .begin_durable(&config(), "order-1", &store)
        .await
        .unwrap();
    let error = tx.commit().await.unwrap_err();
    let DurableTmError::Tm(error @ TmError::UnknownOutcome { .. }) = error else {
        panic!("expected unknown durable end result");
    };
    assert!(tm.recover_unknown(&error).is_err());
    assert_eq!(*store.events.lock().unwrap(), ["begin", "intent", "send"]);
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn begin_source_must_be_confirmed_before_launcher_is_delivered() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        begin(&mut socket, Some("tc:durable")).await;
        peer::eof(&mut socket).await;
    });
    let store = RecordingStore {
        fail_begin: true,
        ..Default::default()
    };
    let result = tm.begin_durable(&config(), "order-1", &store).await;
    assert!(result.is_err());
    assert_eq!(*store.events.lock().unwrap(), ["begin"]);
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}
