//! 协议可选性、保留拒绝事实、旧代际回包隔离及回调预算的真实TCP边界。
#![allow(clippy::unwrap_used)]
#[path = "support/rm_peer.rs"]
mod fixture;
use fixture::{manager, peer, registered};
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchCommitRequest, BranchRegisterRequest,
    BranchRegisterResponse, BranchReportRequest, BranchReportResponse,
};
use seata_protocol::{
    BranchStatus, BranchType, Message, RequestType, ResultCode, TransactionExceptionCode,
};
use seata_rm::{ResourceCatalog, ResourceManager, RmCallError};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
fn request() -> BranchRegisterRequest {
    BranchRegisterRequest {
        xid: Some("tc:9007199254740993".into()),
        branch_type: BranchType::At,
        resource_id: Some("mysql://exact-resource".into()),
        lock_key: None,
        application_data: None,
    }
}

#[tokio::test]
async fn dynamic_resource_buffers_callbacks_and_business_until_the_full_revision_is_ready() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let first = fixture::resource();
    let catalog = ResourceCatalog::new(2, 4096).unwrap();
    catalog.register(first.clone()).unwrap();
    let rm = ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog.clone(),
        fixture::journal().await,
    )
    .build()
    .unwrap();
    let (ready, observed) = tokio::sync::oneshot::channel();
    let second = Arc::new(fixture::Resource {
        key: seata_transport::ResourceKey::new(BranchType::Xa, "mysql://added".into()).unwrap(),
        ..Arc::try_unwrap(fixture::resource()).unwrap()
    });
    let observer = second.clone();
    let (finished, completed) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        ready.send(()).unwrap();
        for resource in ["mysql://exact-resource", "mysql://added"] {
            let value = peer::read(&mut socket).await;
            match &value.body {
                Some(Message::RegisterRMRequest(value)) => {
                    assert_eq!(value.resource_ids.as_deref(), Some(resource))
                }
                other => panic!("business before full revision ready: {other:?}"),
            };
            if resource == "mysql://added" {
                let mut event = callback(-95, 2);
                if let Some(Message::BranchCommitRequest(value)) = &mut event.body {
                    value.branch_type = BranchType::Xa;
                    value.resource_id = resource.into();
                    value.application_data = None;
                }
                peer::write(&mut socket, event).await;
                // PONG证明原连接reader已经读取到回调之后；未完成登记时不得执行或发业务帧。
                peer::write(
                    &mut socket,
                    peer::heartbeat(-96, RequestType::HeartbeatRequest),
                )
                .await;
                let pong = peer::read(&mut socket).await;
                assert_eq!(pong.id, -96);
                assert_eq!(pong.message_type, RequestType::HeartbeatResponse);
                assert_eq!(observer.actions.load(Ordering::SeqCst), 0);
            }
            peer::write(
                &mut socket,
                peer::reply(
                    value.id,
                    Message::RegisterRMResponse(seata_protocol::protocol::RegisterRMResponse {
                        identified: true,
                        wire_version: None,
                    }),
                ),
            )
            .await;
        }
        // 最后一项登记确认后，回调与业务发送并发；两者都必须最终完整出现且各一次。
        let mut callback_seen = false;
        let mut branch_seen = false;
        for _ in 0..2 {
            let event = peer::read(&mut socket).await;
            match event.body {
                Some(Message::BranchRegisterRequest(value)) => {
                    assert!(!branch_seen);
                    branch_seen = true;
                    assert_eq!(value.resource_id.as_deref(), Some("mysql://added"));
                    assert_eq!(value.branch_type, BranchType::Xa);
                    peer::write(
                        &mut socket,
                        peer::reply(
                            event.id,
                            Message::BranchRegisterResponse(BranchRegisterResponse {
                                response: peer::success(),
                                branch_id: 9007199254740995,
                            }),
                        ),
                    )
                    .await;
                }
                Some(Message::BranchCommitResponse(value)) => {
                    assert!(!callback_seen);
                    callback_seen = true;
                    assert_eq!(event.id, -95);
                    assert_eq!(value.branch_status, BranchStatus::PhaseTwoCommitted);
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(callback_seen && branch_seen);
        finished.send(()).unwrap();
        peer::eof(&mut socket).await;
    });
    observed.await.unwrap();
    catalog.register(second.clone()).unwrap();
    let mut value = request();
    value.branch_type = BranchType::Xa;
    value.resource_id = Some("mysql://added".into());
    assert_eq!(
        rm.client().branch_register(value).await.unwrap().branch_id,
        9007199254740995
    );
    completed.await.unwrap();
    assert_eq!(first.actions.load(Ordering::SeqCst), 0);
    assert_eq!(second.actions.load(Ordering::SeqCst), 1);
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn shutdown_reports_resource_cleanup_panic_instead_of_claiming_successful_drain() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let resource = Arc::new(fixture::Resource {
        panic_on_drop: true,
        ..Arc::try_unwrap(fixture::resource()).unwrap()
    });
    let catalog = ResourceCatalog::new(1, 4096).unwrap();
    catalog.register(resource.clone()).unwrap();
    let store = fixture::journal().await;
    let rm = ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog,
        store.clone(),
    )
    .build()
    .unwrap();
    let client = rm.client();
    let mut socket = registered(&listener).await;
    peer::write(&mut socket, callback(-90, 1)).await;
    resource.entered.notified().await;
    let result = rm.shutdown().await;
    peer::eof(&mut socket).await;
    assert!(
        result.is_err(),
        "resource cleanup panic was discarded as successful shutdown"
    );
    assert!(matches!(
        client.branch_register(request()).await,
        Err(RmCallError::LocalFailureBeforeSend { .. })
    ));
    let identity = seata_rm::BranchIdentity::new(
        "tc:9007199254740993".into(),
        1,
        resource.key.clone(),
        Some("original-nonce".into()),
    )
    .unwrap();
    assert_eq!(
        store
            .lease(identity, seata_rm::BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(seata_rm::BranchRecordState::Pending)
    );
}
fn callback(wire: i32, id: i64) -> seata_protocol::protocol::RpcMessage {
    let mut value = peer::reply(
        wire,
        Message::BranchCommitRequest(BranchCommitRequest {
            xid: "tc:9007199254740993".into(),
            branch_id: id,
            branch_type: BranchType::At,
            resource_id: "mysql://exact-resource".into(),
            application_data: Some("original-nonce".into()),
        }),
    );
    value.message_type = RequestType::RequestSync;
    value.head_map = vec![("origin".into(), Some(wire.to_string()))];
    value
}

#[tokio::test]
async fn branch_report_preserves_java_absent_resource_and_application_data_without_filling_them() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let value = peer::read(&mut socket).await;
        match value.body {
            Some(Message::BranchReportRequest(value)) => {
                assert_eq!(value.resource_id, None);
                assert_eq!(value.application_data, None);
                assert_eq!(value.branch_id, 9007199254740995);
            }
            other => panic!("{other:?}"),
        };
        peer::write(
            &mut socket,
            peer::reply(
                value.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: peer::success(),
                }),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    rm.client()
        .branch_report(BranchReportRequest {
            xid: Some("tc:9007199254740993".into()),
            branch_id: 9007199254740995,
            status: BranchStatus::PhaseOneDone,
            resource_id: None,
            application_data: None,
            branch_type: BranchType::At,
        })
        .await
        .unwrap();
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn failed_branch_registration_preserves_returned_id_message_exception_and_original_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let value = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(
                value.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: AbstractTransactionResponse {
                        result_code: ResultCode::Failed,
                        msg: Some("exact-error-message".into()),
                        transaction_exception_code: TransactionExceptionCode::BranchRegisterFailed,
                    },
                    branch_id: 9007199254740995,
                }),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let original = request();
    let error = rm
        .client()
        .branch_register(original.clone())
        .await
        .unwrap_err();
    assert!(!error.is_unknown_outcome());
    assert!(!format!("{error:?}").contains("exact-error-message"));
    match error {
        RmCallError::DefiniteRejected {
            request, response, ..
        } => {
            assert_eq!(*request, Message::BranchRegisterRequest(original));
            match response.body {
                Some(Message::BranchRegisterResponse(value)) => {
                    assert_eq!(value.branch_id, 9007199254740995);
                    assert_eq!(value.response.msg.as_deref(), Some("exact-error-message"));
                    assert_eq!(
                        value.response.transaction_exception_code,
                        TransactionExceptionCode::BranchRegisterFailed
                    );
                }
                other => panic!("{other:?}"),
            };
        }
        other => panic!("{other:?}"),
    };
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn successful_registration_without_positive_issued_id_remains_protocol_uncertainty() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        for id in [0, -1] {
            let value = peer::read(&mut socket).await;
            peer::write(
                &mut socket,
                peer::reply(
                    value.id,
                    Message::BranchRegisterResponse(BranchRegisterResponse {
                        response: peer::success(),
                        branch_id: id,
                    }),
                ),
            )
            .await;
        }
        peer::eof(&mut socket).await;
    });
    for id in [0, -1] {
        let error = rm.client().branch_register(request()).await.unwrap_err();
        assert!(error.is_unknown_outcome());
        match error {
            RmCallError::ProtocolFailure {
                response: Some(response),
                may_have_effect: true,
                ..
            } => match response.body {
                Some(Message::BranchRegisterResponse(value)) => assert_eq!(value.branch_id, id),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
    }
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn callback_started_on_old_connection_never_sends_its_reply_on_the_replacement_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let resource = Arc::new(fixture::Resource {
        pause: true,
        ..Arc::try_unwrap(fixture::resource()).unwrap()
    });
    let catalog = ResourceCatalog::new(1, 4096).unwrap();
    catalog.register(resource.clone()).unwrap();
    let rm = ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog,
        fixture::journal().await,
    )
    .build()
    .unwrap();
    let mut original = registered(&listener).await;
    peer::write(&mut original, callback(-80, 1)).await;
    resource.entered.notified().await;
    drop(original);
    let mut replacement = tokio::time::timeout(Duration::from_secs(3), registered(&listener))
        .await
        .unwrap();
    let new_request = callback(-81, 1);
    peer::write(&mut replacement, new_request.clone()).await;
    resource.release.notify_one();
    let result = peer::read(&mut replacement).await;
    assert_eq!(result.id, -81);
    assert_eq!(result.head_map, new_request.head_map);
    match result.body {
        Some(Message::BranchCommitResponse(value)) => {
            assert_eq!(value.branch_status, BranchStatus::PhaseTwoCommitted);
            assert_eq!(value.xid.as_deref(), Some("tc:9007199254740993"));
            assert_eq!(value.branch_id, 1);
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
    rm.shutdown().await.unwrap();
    peer::eof(&mut replacement).await;
}

#[tokio::test]
async fn callback_worker_budget_one_does_not_start_queued_resource_actions_during_shutdown() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let resource = Arc::new(fixture::Resource {
        stall: true,
        ..Arc::try_unwrap(fixture::resource()).unwrap()
    });
    let catalog = ResourceCatalog::new(1, 4096).unwrap();
    catalog.register(resource.clone()).unwrap();
    let rm = ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog,
        fixture::journal().await,
    )
    .maximum_workers(1)
    .build()
    .unwrap();
    let mut socket = registered(&listener).await;
    for id in 1..=3 {
        peer::write(&mut socket, callback(-(id as i32), id)).await;
    }
    resource.entered.notified().await;
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
    assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
}
