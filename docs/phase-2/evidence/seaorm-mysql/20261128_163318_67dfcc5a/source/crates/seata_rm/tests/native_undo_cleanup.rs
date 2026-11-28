//! 原 TCP 清理能力、取消及同步形状拒绝；资源为明确的行为夹具。
#![allow(clippy::unwrap_used)]
#[path = "support/rm_peer.rs"]
mod fixture;
use fixture::peer;
use seata_protocol::{transaction::UndoLogDeleteRequest, BranchType, Message, RequestType};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchResolution, BranchResource, ResourceCatalog,
    ResourceManager, RmError, UndoLogDeleteReceipt,
};
use seata_transport::ResourceKey;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::net::TcpListener;

#[derive(Debug)]
struct Resource {
    key: ResourceKey,
    days: AtomicU64,
    cancelled: Arc<AtomicBool>,
    stall: bool,
}
struct Cancel(Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
impl BranchResource for Resource {
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn delete_expired_undo<'a>(
        &'a self,
        receipt: &'a UndoLogDeleteReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<u64, RmError>> + Send + 'a>> {
        Box::pin(async move {
            assert_eq!(receipt.resource(), &self.key);
            self.days
                .store(u64::from(receipt.save_days()), Ordering::SeqCst);
            let _guard = Cancel(self.cancelled.clone());
            if self.stall {
                std::future::pending::<()>().await;
            }
            Err(RmError::Unavailable {
                reason: "explicit cleanup fixture error",
            })
        })
    }
    fn apply<'a>(
        &'a self,
        _: &'a BranchIdentity,
        _: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(async { Ok(BranchResolution::Applied) })
    }
    fn recover<'a>(
        &'a self,
        _: &'a BranchIdentity,
        _: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(async { Ok(BranchResolution::Unknown) })
    }
}

#[tokio::test]
async fn resource_error_and_deadline_cancel_do_not_emit_reply_or_disconnect() {
    for stall in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let resource = Arc::new(Resource {
            key: ResourceKey::new(BranchType::At, "mysql://exact-resource".into()).unwrap(),
            days: AtomicU64::new(0),
            cancelled: Arc::new(AtomicBool::new(false)),
            stall,
        });
        let catalog = ResourceCatalog::new(4, 4096).unwrap();
        catalog.register(resource.clone()).unwrap();
        let rm = ResourceManager::builder(
            fixture::settings(listener.local_addr().unwrap()),
            fixture::registration(),
            catalog,
            fixture::journal().await,
        )
        .callback_timeout(Duration::from_millis(30))
        .build()
        .unwrap();
        let mut socket = fixture::registered(&listener).await;
        let mut request = peer::reply(
            44,
            Message::UndoLogDeleteRequest(UndoLogDeleteRequest {
                branch_type: BranchType::At,
                resource_id: "mysql://exact-resource".into(),
                save_days: 0,
            }),
        );
        request.message_type = RequestType::RequestOneway;
        peer::write(&mut socket, request).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(100), peer::read(&mut socket))
                .await
                .is_err()
        );
        assert_eq!(resource.days.load(Ordering::SeqCst), 7);
        assert!(resource.cancelled.load(Ordering::SeqCst));
        rm.shutdown().await.unwrap();
        peer::eof(&mut socket).await;
    }
}

#[tokio::test]
async fn synchronous_cleanup_does_not_grant_a_retention_capability() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = fixture::manager(&listener, Duration::from_secs(1)).await;
    let mut socket = fixture::registered(&listener).await;
    let mut request = peer::reply(
        45,
        Message::UndoLogDeleteRequest(UndoLogDeleteRequest {
            branch_type: BranchType::At,
            resource_id: "mysql://exact-resource".into(),
            save_days: 1,
        }),
    );
    request.message_type = RequestType::RequestSync;
    peer::write(&mut socket, request).await;
    peer::eof(&mut socket).await;
    assert_eq!(resource.actions.load(Ordering::SeqCst), 0);
    rm.shutdown().await.unwrap();
}
