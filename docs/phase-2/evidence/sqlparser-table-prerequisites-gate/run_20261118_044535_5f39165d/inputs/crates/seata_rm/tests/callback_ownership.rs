//! 仅原网络回调获得登记能力；检查取消前的持久 Pending 和整批校验边界。
#![allow(clippy::unwrap_used)]
#[path = "support/rm_peer.rs"]
mod fixture;
use seata_protocol::protocol::MergedWarpMessage;
use seata_protocol::transaction::{BranchRollbackRequest, GlobalStatusRequest};
use seata_protocol::{BranchStatus, BranchType, Message, RequestType, ResultCode};
use seata_rm::{
    BranchCallbackObserver, BranchCallbackReceipt, BranchDispatcher, BranchIdentity, BranchPhase,
    BranchRecordState, BranchResolution, BranchResource, FileBranchStore, ResourceCatalog,
    ResourceManager, RmError,
};
use seata_transport::ResourceKey;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

#[derive(Debug)]
struct ObservedResource {
    key: ResourceKey,
    observations: AtomicU64,
    applies: AtomicU64,
    recoveries: AtomicU64,
    stall: AtomicBool,
    entered: tokio::sync::Notify,
}
impl BranchCallbackObserver for ObservedResource {
    fn observe<'a>(
        &'a self,
        receipt: &'a BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), RmError>> + Send + 'a>> {
        assert_eq!(receipt.identity().resource(), &self.key);
        assert_eq!(receipt.phase(), BranchPhase::Rollback);
        self.observations.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            self.entered.notify_one();
            if self.stall.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            Ok(())
        })
    }
}
impl BranchResource for ObservedResource {
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn callback_observer(&self) -> Option<&dyn BranchCallbackObserver> {
        Some(self)
    }
    fn apply<'a>(
        &'a self,
        _identity: &'a BranchIdentity,
        _phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        self.applies.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(BranchResolution::Applied) })
    }
    fn recover<'a>(
        &'a self,
        _identity: &'a BranchIdentity,
        _phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        self.recoveries.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(BranchResolution::Applied) })
    }
}
fn resource(stall: bool) -> Arc<ObservedResource> {
    Arc::new(ObservedResource {
        key: ResourceKey::new(BranchType::At, "mysql://exact-resource".into()).unwrap(),
        observations: AtomicU64::new(0),
        applies: AtomicU64::new(0),
        recoveries: AtomicU64::new(0),
        stall: AtomicBool::new(stall),
        entered: tokio::sync::Notify::new(),
    })
}
fn identity(resource: &ObservedResource) -> BranchIdentity {
    BranchIdentity::new(
        "callback:authority".into(),
        9007199254740995,
        resource.key.clone(),
        None,
    )
    .unwrap()
}
fn catalog(resource: Arc<ObservedResource>) -> ResourceCatalog {
    let value = ResourceCatalog::new(4, 4096).unwrap();
    value.register(resource).unwrap();
    value
}
fn callback() -> Message {
    Message::BranchRollbackRequest(BranchRollbackRequest {
        xid: "callback:authority".into(),
        branch_id: 9007199254740995,
        branch_type: BranchType::At,
        resource_id: "mysql://exact-resource".into(),
        application_data: None,
    })
}
fn frame(wire: i32, body: Message) -> seata_protocol::protocol::RpcMessage {
    let mut value = fixture::peer::reply(wire, body);
    value.message_type = RequestType::RequestSync;
    value
}
fn assert_response(
    value: seata_protocol::protocol::RpcMessage,
    wire: i32,
    result: ResultCode,
    status: BranchStatus,
) {
    assert_eq!(value.id, wire);
    match value.body {
        Some(Message::BranchRollbackResponse(v)) => {
            assert_eq!(v.response.result_code, result);
            assert_eq!(v.branch_status, status);
            assert_eq!(v.branch_id, 9007199254740995);
            assert_eq!(v.xid.as_deref(), Some("callback:authority"));
        }
        other => panic!("{other:?}"),
    }
}
fn manager(
    listener: &TcpListener,
    resource: Arc<ObservedResource>,
    store: FileBranchStore,
) -> ResourceManager {
    ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog(resource),
        store,
    )
    .callback_timeout(Duration::from_millis(250))
    .build()
    .unwrap()
}

#[tokio::test]
async fn cancelled_observer_leaves_pending_and_next_callback_recovers_without_apply() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let resource = resource(true);
    let store = fixture::journal().await;
    let rm = manager(&listener, resource.clone(), store.clone());
    let mut socket = fixture::registered(&listener).await;
    fixture::peer::write(&mut socket, frame(-93, callback())).await;
    tokio::time::timeout(Duration::from_secs(2), resource.entered.notified())
        .await
        .unwrap();
    // 租约持有期间无法读日志；必须等真实回调超时释放，再确认 Pending 已持久化。
    assert_response(
        fixture::peer::read(&mut socket).await,
        -93,
        ResultCode::Failed,
        BranchStatus::PhaseTwoRollbackFailedRetryable,
    );
    assert_eq!(
        store
            .lease(identity(&resource), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    assert_eq!(resource.applies.load(Ordering::SeqCst), 0);
    assert_eq!(resource.recoveries.load(Ordering::SeqCst), 0);
    resource.stall.store(false, Ordering::SeqCst);
    fixture::peer::write(&mut socket, frame(-94, callback())).await;
    assert_response(
        fixture::peer::read(&mut socket).await,
        -94,
        ResultCode::Success,
        BranchStatus::PhaseTwoRollbacked,
    );
    // 已完成重复不再触发来源登记，也不重复资源动作。
    fixture::peer::write(&mut socket, frame(-95, callback())).await;
    assert_response(
        fixture::peer::read(&mut socket).await,
        -95,
        ResultCode::Success,
        BranchStatus::PhaseTwoRollbacked,
    );
    assert_eq!(resource.observations.load(Ordering::SeqCst), 2);
    assert_eq!(resource.applies.load(Ordering::SeqCst), 0);
    assert_eq!(resource.recoveries.load(Ordering::SeqCst), 1);
    assert_eq!(
        store
            .lease(identity(&resource), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    rm.shutdown().await.unwrap();
}

#[tokio::test]
async fn manual_dispatch_does_not_invoke_a_resource_callback_observer() {
    let resource = resource(true);
    let dispatcher = BranchDispatcher::new(catalog(resource.clone()), fixture::journal().await);
    assert_eq!(
        dispatcher
            .dispatch(identity(&resource), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchStatus::PhaseTwoRollbacked
    );
    assert_eq!(resource.observations.load(Ordering::SeqCst), 0);
    assert_eq!(resource.applies.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn invalid_later_batch_child_cannot_issue_first_child_callback_receipt() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let resource = resource(false);
    let store = fixture::journal().await;
    let rm = manager(&listener, resource.clone(), store.clone());
    let mut socket = fixture::registered(&listener).await;
    fixture::peer::write(
        &mut socket,
        frame(
            -96,
            Message::MergedWarpMessage(MergedWarpMessage {
                msgs: vec![
                    callback(),
                    Message::GlobalStatusRequest(GlobalStatusRequest {
                        xid: Some("callback:authority".into()),
                        extra_data: None,
                    }),
                ],
                msg_ids: vec![],
            }),
        ),
    )
    .await;
    fixture::peer::eof(&mut socket).await;
    assert_eq!(resource.observations.load(Ordering::SeqCst), 0);
    assert_eq!(resource.applies.load(Ordering::SeqCst), 0);
    assert_eq!(resource.recoveries.load(Ordering::SeqCst), 0);
    assert_eq!(
        store
            .lease(identity(&resource), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        None
    );
    rm.shutdown().await.unwrap();
}
