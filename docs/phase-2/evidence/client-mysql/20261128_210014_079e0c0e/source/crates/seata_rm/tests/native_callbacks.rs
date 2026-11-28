//! 真正TCP回调验证原连接身份、合并批次、失败及关闭，资源夹具不是SQL实现。
#![allow(clippy::unwrap_used)]
#[path = "support/rm_peer.rs"]
mod fixture;
use fixture::{manager, peer, registered};
use seata_protocol::protocol::{MergedWarpMessage, RpcMessage};
use seata_protocol::transaction::{
    BranchCommitRequest, BranchRollbackRequest, GlobalStatusRequest,
};
use seata_protocol::{BranchType, Message, RequestType, ResultCode};
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, ResourceCatalog, ResourceManager};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

fn callback(commit: bool, id: i64, resource: &str) -> Message {
    if commit {
        Message::BranchCommitRequest(BranchCommitRequest {
            xid: "tc:9007199254740993".into(),
            branch_id: id,
            branch_type: BranchType::At,
            resource_id: resource.into(),
            application_data: Some("native-original-nonce".into()),
        })
    } else {
        Message::BranchRollbackRequest(BranchRollbackRequest {
            xid: "tc:9007199254740993".into(),
            branch_id: id,
            branch_type: BranchType::At,
            resource_id: resource.into(),
            application_data: Some("native-original-nonce".into()),
        })
    }
}
fn frame(id: i32, body: Message) -> RpcMessage {
    let mut value = peer::reply(id, body);
    value.message_type = RequestType::RequestSync;
    value.head_map = vec![
        ("nullable".into(), None),
        ("empty".into(), Some(String::new())),
    ];
    value
}
fn fields(value: &Message) -> (ResultCode, i64, u8) {
    match value {
        Message::BranchCommitResponse(v) => {
            assert_eq!(v.xid.as_deref(), Some("tc:9007199254740993"));
            (
                v.response.result_code,
                v.branch_id,
                v.branch_status.to_java_v1(),
            )
        }
        Message::BranchRollbackResponse(v) => {
            assert_eq!(v.xid.as_deref(), Some("tc:9007199254740993"));
            (
                v.response.result_code,
                v.branch_id,
                v.branch_status.to_java_v1(),
            )
        }
        _ => panic!("wrong response: {value:?}"),
    }
}

#[tokio::test]
async fn original_oneway_undo_cleanup_has_no_reply_and_keeps_the_registered_channel_usable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    for (kind, id) in [
        (BranchType::At, "mysql://exact-resource"),
        (BranchType::At, "missing"),
        (BranchType::Tcc, "mysql://exact-resource"),
    ] {
        let mut request = frame(
            70,
            Message::UndoLogDeleteRequest(seata_protocol::transaction::UndoLogDeleteRequest {
                branch_type: kind,
                resource_id: id.into(),
                save_days: 7,
            }),
        );
        request.message_type = RequestType::RequestOneway;
        peer::write(&mut socket, request).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(40), peer::read(&mut socket))
                .await
                .is_err(),
            "oneway cleanup replied or closed its channel"
        );
    }
    peer::write(
        &mut socket,
        frame(71, callback(true, 99, "mysql://exact-resource")),
    )
    .await;
    let response = peer::read(&mut socket).await;
    assert_eq!(response.id, 71);
    assert_eq!(fields(response.body.as_ref().unwrap()).2, 5);
    assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
    rm.shutdown().await.unwrap();
}

#[tokio::test]
async fn duplicate_callbacks_keep_signed_id_nullable_headers_and_exact_branch_identity_without_reexecution(
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    for (wire, commit, id, status, success) in [
        (i32::MIN, true, 9007199254740995, 5, true),
        (-11, true, 9007199254740995, 5, true),
        (-12, false, 9007199254740995, 10, false),
        (-13, false, 9007199254740996, 8, true),
    ] {
        let request = frame(wire, callback(commit, id, "mysql://exact-resource"));
        peer::write(&mut socket, request.clone()).await;
        let response = peer::read(&mut socket).await;
        assert_eq!(response.id, wire);
        assert_eq!(response.head_map, request.head_map);
        assert_eq!(
            fields(response.body.as_ref().unwrap()),
            (
                if success {
                    ResultCode::Success
                } else {
                    ResultCode::Failed
                },
                id,
                status
            )
        );
    }
    assert_eq!(resource.actions.load(Ordering::SeqCst), 2);
    let metrics = rm.metrics_sampler();
    let commit = metrics.snapshot(BranchPhase::Commit);
    let rollback = metrics.snapshot(BranchPhase::Rollback);
    assert_eq!(
        (
            commit.applied,
            commit.duplicate_completed,
            commit.apply_attempts,
            commit.journal_completed
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(
        (
            rollback.applied,
            rollback.errors,
            rollback.apply_attempts,
            rollback.journal_completed
        ),
        (1, 1, 1, 1)
    );
    rm.shutdown().await.unwrap();
    assert!(metrics.snapshot(BranchPhase::Commit).journal_closed);
    peer::eof(&mut socket).await;
}

#[tokio::test]
async fn missing_or_wrong_branch_type_resource_returns_explicit_failure_with_original_identity() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    let mut wrong = callback(true, 1, "mysql://exact-resource");
    if let Message::BranchCommitRequest(value) = &mut wrong {
        value.branch_type = BranchType::Xa;
    }
    for (id, body) in [(17, callback(true, 1, "mysql://missing")), (18, wrong)] {
        peer::write(&mut socket, frame(id, body)).await;
        let response = peer::read(&mut socket).await;
        assert_eq!(
            fields(response.body.as_ref().unwrap()),
            (ResultCode::Failed, 1, 6)
        );
    }
    assert_eq!(resource.actions.load(Ordering::SeqCst), 0);
    rm.shutdown().await.unwrap();
}

#[tokio::test]
async fn flat_batch_preserves_order_and_exact_duplicate_children_apply_only_once() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    let body = Message::MergedWarpMessage(MergedWarpMessage {
        msgs: vec![
            callback(true, 1, "mysql://exact-resource"),
            callback(true, 1, "mysql://exact-resource"),
            callback(false, 2, "mysql://exact-resource"),
        ],
        msg_ids: vec![i32::MIN, -2, i32::MAX],
    });
    peer::write(&mut socket, frame(-25, body)).await;
    let response = peer::read(&mut socket).await;
    assert_eq!(response.id, -25);
    match response.body {
        Some(Message::MergeResultMessage(value)) => assert_eq!(
            value.msgs.unwrap().iter().map(fields).collect::<Vec<_>>(),
            vec![
                (ResultCode::Success, 1, 5),
                (ResultCode::Success, 1, 5),
                (ResultCode::Success, 2, 8)
            ]
        ),
        other => panic!("{other:?}"),
    };
    assert_eq!(resource.actions.load(Ordering::SeqCst), 2);
    rm.shutdown().await.unwrap();
}

#[tokio::test]
async fn an_unsupported_later_batch_child_rejects_before_the_first_resource_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    let body = Message::MergedWarpMessage(MergedWarpMessage {
        msgs: vec![
            callback(true, 1, "mysql://exact-resource"),
            Message::GlobalStatusRequest(GlobalStatusRequest {
                xid: Some("tc:1".into()),
                extra_data: None,
            }),
        ],
        msg_ids: vec![],
    });
    peer::write(&mut socket, frame(-26, body)).await;
    peer::eof(&mut socket).await;
    assert_eq!(resource.actions.load(Ordering::SeqCst), 0);
    rm.shutdown().await.unwrap();
}

#[tokio::test]
async fn contradictory_directions_inside_one_batch_reject_before_any_resource_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    let body = Message::MergedWarpMessage(MergedWarpMessage {
        msgs: vec![
            callback(true, 1, "mysql://exact-resource"),
            callback(false, 1, "mysql://exact-resource"),
        ],
        msg_ids: vec![],
    });
    peer::write(&mut socket, frame(-27, body)).await;
    peer::eof(&mut socket).await;
    assert_eq!(resource.actions.load(Ordering::SeqCst), 0);
    rm.shutdown().await.unwrap();
}

async fn stalled(shutdown: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let resource = Arc::new(fixture::Resource {
        stall: true,
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
    .callback_timeout(Duration::from_millis(250))
    .build()
    .unwrap();
    let mut socket = registered(&listener).await;
    peer::write(
        &mut socket,
        frame(-28, callback(true, 1, "mysql://exact-resource")),
    )
    .await;
    resource.entered.notified().await;
    let metrics = rm.metrics_sampler();
    if shutdown {
        rm.shutdown().await.unwrap();
        peer::eof(&mut socket).await;
    } else {
        let response = peer::read(&mut socket).await;
        assert_eq!(
            fields(response.body.as_ref().unwrap()),
            (ResultCode::Failed, 1, 6)
        );
        peer::write(
            &mut socket,
            frame(-29, callback(true, 1, "mysql://exact-resource")),
        )
        .await;
        let response = peer::read(&mut socket).await;
        assert_eq!(
            fields(response.body.as_ref().unwrap()),
            (ResultCode::Success, 1, 6)
        );
        rm.shutdown().await.unwrap();
    }
    assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
    let counts = metrics.snapshot(BranchPhase::Commit);
    assert_eq!(
        (
            counts.active,
            counts.cancelled,
            counts.apply_attempts,
            counts.journal_pending
        ),
        (0, 1, 1, 1)
    );
    assert_eq!(
        (counts.unknown, counts.recover_attempts),
        if shutdown { (0, 0) } else { (1, 1) }
    );
    let identity = BranchIdentity::new(
        "tc:9007199254740993".into(),
        1,
        resource.key.clone(),
        Some("native-original-nonce".into()),
    )
    .unwrap();
    assert_eq!(
        store
            .lease(identity, BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
}
#[tokio::test]
async fn callback_deadline_cancels_resource_future_and_next_instruction_recovers_without_replaying()
{
    stalled(false).await;
}
#[tokio::test]
async fn shutdown_owns_cancelled_resource_tasks_and_leaves_durable_pending() {
    stalled(true).await;
}

#[tokio::test]
async fn dropping_manager_synchronously_closes_the_pool_for_all_client_clones() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let client = rm.client();
    let mut socket = registered(&listener).await;
    drop(rm);
    peer::eof(&mut socket).await;
    let request = seata_protocol::transaction::BranchRegisterRequest {
        xid: Some("tc:1".into()),
        branch_type: BranchType::At,
        resource_id: Some("mysql://exact-resource".into()),
        lock_key: None,
        application_data: None,
    };
    assert!(matches!(
        client.branch_register(request).await,
        Err(seata_rm::RmCallError::LocalFailureBeforeSend { .. })
    ));
}

#[tokio::test]
async fn shutdown_reports_an_external_journal_lease_instead_of_claiming_drain() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let resource = fixture::resource();
    let catalog = ResourceCatalog::new(1, 4096).unwrap();
    catalog.register(resource.clone()).unwrap();
    let store = fixture::journal().await;
    let identity =
        BranchIdentity::new("tc:external-lease".into(), 91, resource.key.clone(), None).unwrap();
    let lease = store
        .lease(identity.clone(), BranchPhase::Commit)
        .await
        .unwrap();
    let rm = ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog,
        store.clone(),
    )
    .callback_timeout(Duration::from_millis(100))
    .build()
    .unwrap();
    let client = rm.client();
    let mut socket = registered(&listener).await;
    let result = tokio::time::timeout(Duration::from_secs(2), rm.shutdown())
        .await
        .unwrap();
    assert!(matches!(
        result,
        Err(seata_rm::RmError::Unavailable {
            reason: "RM journal work has not drained"
        })
    ));
    peer::eof(&mut socket).await;
    drop(client);
    drop(lease);
    tokio::time::timeout(
        Duration::from_secs(2),
        store.lease(identity, BranchPhase::Commit),
    )
    .await
    .unwrap()
    .unwrap();
}
