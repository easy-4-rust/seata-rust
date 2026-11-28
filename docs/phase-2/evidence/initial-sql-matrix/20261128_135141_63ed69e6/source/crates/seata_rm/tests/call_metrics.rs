//! RM 出站结果与客户端克隆计数，保留原字段及写入边界。
#![allow(clippy::unwrap_used)]
#[path = "support/rm_peer.rs"]
mod fixture;
use fixture::{manager, peer, registered};
use seata_protocol::transaction::{
    BranchRegisterRequest, BranchRegisterResponse, BranchReportRequest, BranchReportResponse,
    GlobalLockQueryRequest, GlobalLockQueryResponse,
};
use seata_protocol::{BranchStatus, BranchType, Message, ResultCode};
use seata_rm::{ResourceClient, RmCallError, RmOperation};
use std::time::Duration;
use tokio::{net::TcpListener, sync::oneshot};

fn registration() -> BranchRegisterRequest {
    BranchRegisterRequest {
        xid: Some("tc:metrics-private-id".into()),
        branch_type: BranchType::At,
        resource_id: Some("mysql://exact-resource".into()),
        lock_key: Some("work:1".into()),
        application_data: Some("private-application-credential".into()),
    }
}
fn report() -> BranchReportRequest {
    BranchReportRequest {
        xid: registration().xid,
        branch_id: 9007199254740995,
        branch_type: BranchType::At,
        resource_id: None,
        status: BranchStatus::PhaseOneDone,
        application_data: None,
    }
}
fn query() -> GlobalLockQueryRequest {
    let value = registration();
    GlobalLockQueryRequest {
        xid: value.xid,
        branch_type: value.branch_type,
        resource_id: value.resource_id,
        lock_key: value.lock_key,
        application_data: value.application_data,
    }
}
async fn issue(client: &ResourceClient, operation: RmOperation) -> Result<(), RmCallError> {
    match operation {
        RmOperation::BranchRegister => client
            .branch_register_with_receipt(registration())
            .await
            .map(|_| ()),
        RmOperation::BranchReport => client.branch_report(report()).await.map(|_| ()),
        RmOperation::LockQuery => client.lock_query(query()).await.map(|_| ()),
    }
}
fn response(operation: RmOperation, failed: bool, branch_id: i64) -> Message {
    let mut prefix = peer::success();
    if failed {
        prefix.result_code = ResultCode::Failed;
        prefix.msg = Some("private-tc-credential".into());
    }
    match operation {
        RmOperation::BranchRegister => Message::BranchRegisterResponse(BranchRegisterResponse {
            response: prefix,
            branch_id,
        }),
        RmOperation::BranchReport => {
            Message::BranchReportResponse(BranchReportResponse { response: prefix })
        }
        RmOperation::LockQuery => Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
            response: prefix,
            lockable: false,
        }),
    }
}

#[tokio::test]
async fn clones_share_single_receipt_and_lock_false_success_then_closed_calls_are_not_sent() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let client = rm.client();
    let clone = client.clone();
    let metrics = rm.call_metrics_sampler();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        for operation in [
            RmOperation::BranchRegister,
            RmOperation::BranchReport,
            RmOperation::LockQuery,
        ] {
            let request = peer::read(&mut socket).await;
            if let Some(Message::BranchReportRequest(value)) = &request.body {
                assert!(value.resource_id.is_none());
                assert!(value.application_data.is_none());
            }
            peer::write(
                &mut socket,
                peer::reply(request.id, response(operation, false, 9007199254740995)),
            )
            .await;
        }
        peer::eof(&mut socket).await;
    });
    let receipt = client
        .branch_register_with_receipt(registration())
        .await
        .unwrap();
    assert_eq!(receipt.identity().branch_id(), 9007199254740995);
    clone.branch_report(report()).await.unwrap();
    assert!(!client.lock_query(query()).await.unwrap().lockable);
    let mut invalid = registration();
    invalid.xid = None;
    assert!(matches!(
        clone.branch_register_with_receipt(invalid).await,
        Err(RmCallError::Invalid { .. })
    ));
    for operation in [
        RmOperation::BranchRegister,
        RmOperation::BranchReport,
        RmOperation::LockQuery,
    ] {
        let counts = metrics.snapshot(operation);
        assert_eq!((counts.active, counts.observed, counts.unknown), (0, 1, 0));
    }
    assert_eq!(
        metrics.snapshot(RmOperation::BranchRegister).local_rejected,
        1
    );
    rm.shutdown().await.unwrap();
    server.await.unwrap();
    assert!(matches!(
        clone.branch_report(report()).await,
        Err(RmCallError::LocalFailureBeforeSend { .. })
    ));
    assert_eq!(metrics.snapshot(RmOperation::BranchReport).not_sent, 1);
    assert_eq!(
        client
            .call_metrics_sampler()
            .snapshot(RmOperation::BranchReport),
        metrics.snapshot(RmOperation::BranchReport)
    );
}

#[tokio::test]
async fn failed_responses_and_invalid_issued_ids_do_not_count_as_observed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let metrics = rm.call_metrics_sampler();
    let client = rm.client();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        for operation in [
            RmOperation::BranchRegister,
            RmOperation::BranchReport,
            RmOperation::LockQuery,
        ] {
            let request = peer::read(&mut socket).await;
            peer::write(
                &mut socket,
                peer::reply(request.id, response(operation, true, 42)),
            )
            .await;
        }
        let request = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(request.id, response(RmOperation::BranchRegister, false, 0)),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    for operation in [
        RmOperation::BranchRegister,
        RmOperation::BranchReport,
        RmOperation::LockQuery,
    ] {
        assert!(matches!(
            issue(&client, operation).await,
            Err(RmCallError::DefiniteRejected { .. })
        ));
        assert_eq!(metrics.snapshot(operation).tc_rejected, 1);
        assert_eq!(metrics.snapshot(operation).observed, 0);
    }
    assert!(issue(&client, RmOperation::BranchRegister)
        .await
        .unwrap_err()
        .is_unknown_outcome());
    let counts = metrics.snapshot(RmOperation::BranchRegister);
    assert_eq!(
        (counts.protocol_errors, counts.unknown, counts.observed),
        (1, 1, 0)
    );
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn lost_responses_keep_unknown_once_without_automatic_replay() {
    for operation in [
        RmOperation::BranchRegister,
        RmOperation::BranchReport,
        RmOperation::LockQuery,
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, _) = manager(&listener, Duration::from_millis(150)).await;
        let metrics = rm.call_metrics_sampler();
        let client = rm.client();
        let server = tokio::spawn(async move {
            let mut socket = registered(&listener).await;
            let _ = peer::read(&mut socket).await;
            peer::eof(&mut socket).await;
        });
        assert!(issue(&client, operation)
            .await
            .unwrap_err()
            .is_unknown_outcome());
        let counts = metrics.snapshot(operation);
        assert_eq!(
            (
                counts.active,
                counts.unknown,
                counts.observed,
                counts.tc_rejected
            ),
            (0, 1, 0, 0)
        );
        rm.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn cancellation_releases_active_without_inventing_a_result() {
    for operation in [
        RmOperation::BranchRegister,
        RmOperation::BranchReport,
        RmOperation::LockQuery,
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
        let metrics = rm.call_metrics_sampler();
        let client = rm.client();
        let (seen_tx, seen) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = registered(&listener).await;
            let _ = peer::read(&mut socket).await;
            seen_tx.send(()).unwrap();
            peer::eof(&mut socket).await;
        });
        let mut call = Box::pin(issue(&client, operation));
        tokio::select! {result=&mut call=>panic!("{result:?}"),_=seen=>{}}
        assert_eq!(metrics.snapshot(operation).active, 1);
        drop(call);
        let counts = metrics.snapshot(operation);
        assert_eq!(
            (
                counts.active,
                counts.cancelled,
                counts.unknown,
                counts.local_rejected
            ),
            (0, 1, 0, 0)
        );
        rm.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn unavailable_endpoint_has_explicit_not_sent_result() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    drop(listener);
    let catalog = seata_rm::ResourceCatalog::new(4, 4096).unwrap();
    catalog.register(fixture::resource()).unwrap();
    let rm = seata_rm::ResourceManager::builder(
        fixture::settings(endpoint),
        fixture::registration(),
        catalog,
        fixture::journal().await,
    )
    .request_timeout(Duration::from_millis(30))
    .build()
    .unwrap();
    let metrics = rm.call_metrics_sampler();
    assert!(!rm
        .client()
        .lock_query(query())
        .await
        .unwrap_err()
        .is_unknown_outcome());
    assert_eq!(metrics.snapshot(RmOperation::LockQuery).not_sent, 1);
    rm.shutdown().await.unwrap();
}
