//! 真实TCP验证正式RM单次出站调用、字段保留、错误分类与关闭。
#![allow(clippy::unwrap_used)]
#[path = "support/rm_peer.rs"]
mod fixture;
use fixture::{manager, peer, registered};
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchRegisterRequest, BranchRegisterResponse,
    BranchReportRequest, BranchReportResponse, GlobalLockQueryRequest, GlobalLockQueryResponse,
};
use seata_protocol::{BranchStatus, BranchType, Message, ResultCode, TransactionExceptionCode};
use seata_rm::RmCallError;
use std::time::Duration;
use tokio::net::TcpListener;

fn request() -> BranchRegisterRequest {
    BranchRegisterRequest {
        xid: Some("tc:9007199254740993".into()),
        branch_type: BranchType::At,
        resource_id: Some("mysql://exact-resource".into()),
        lock_key: Some("orders:9007199254740995".into()),
        application_data: Some("original-nonce".into()),
    }
}

#[tokio::test]
async fn registered_rm_registers_reports_and_observes_lock_false_without_losing_fields() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let expected = request();
    let original = expected.clone();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let value = peer::read(&mut socket).await;
        assert_eq!(value.body, Some(Message::BranchRegisterRequest(expected)));
        peer::write(
            &mut socket,
            peer::reply(
                value.id,
                Message::BranchRegisterResponse(BranchRegisterResponse {
                    response: peer::success(),
                    branch_id: 9007199254740995,
                }),
            ),
        )
        .await;
        let value = peer::read(&mut socket).await;
        match value.body {
            Some(Message::BranchReportRequest(value)) => {
                assert_eq!(value.branch_id, 9007199254740995);
                assert_eq!(value.xid.as_deref(), Some("tc:9007199254740993"));
                assert_eq!(value.application_data.as_deref(), Some("original-nonce"));
                assert_eq!(value.status, BranchStatus::PhaseOneDone);
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
        let value = peer::read(&mut socket).await;
        assert!(matches!(
            value.body,
            Some(Message::GlobalLockQueryRequest(_))
        ));
        peer::write(
            &mut socket,
            peer::reply(
                value.id,
                Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
                    response: peer::success(),
                    lockable: false,
                }),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let client = rm.client();
    let registered = client.branch_register(original.clone()).await.unwrap();
    assert_eq!(registered.branch_id, 9007199254740995);
    client
        .branch_report(BranchReportRequest {
            xid: original.xid.clone(),
            branch_id: registered.branch_id,
            status: BranchStatus::PhaseOneDone,
            resource_id: original.resource_id.clone(),
            application_data: original.application_data.clone(),
            branch_type: original.branch_type,
        })
        .await
        .unwrap();
    assert!(
        !client
            .lock_query(GlobalLockQueryRequest {
                xid: original.xid,
                branch_type: original.branch_type,
                resource_id: original.resource_id,
                lock_key: original.lock_key,
                application_data: original.application_data
            })
            .await
            .unwrap()
            .lockable
    );
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn failed_lock_query_keeps_true_field_and_error_prefix_instead_of_returning_a_boolean() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let value = peer::read(&mut socket).await;
        peer::write(
            &mut socket,
            peer::reply(
                value.id,
                Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
                    response: AbstractTransactionResponse {
                        result_code: ResultCode::Failed,
                        msg: None,
                        transaction_exception_code: TransactionExceptionCode::LockableCheckFailed,
                    },
                    lockable: true,
                }),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let value = request();
    let error = rm
        .client()
        .lock_query(GlobalLockQueryRequest {
            xid: value.xid,
            branch_type: value.branch_type,
            resource_id: value.resource_id,
            lock_key: value.lock_key,
            application_data: value.application_data,
        })
        .await
        .unwrap_err();
    match error {
        RmCallError::DefiniteRejected { response, .. } => match response.body {
            Some(Message::GlobalLockQueryResponse(value)) => {
                assert!(value.lockable);
                assert_eq!(value.response.msg, None);
                assert_eq!(
                    value.response.transaction_exception_code,
                    TransactionExceptionCode::LockableCheckFailed
                );
            }
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn lost_registration_response_preserves_original_request_and_reconnect_never_replays_it() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_millis(400)).await;
    let expected = request();
    let original = expected.clone();
    let (seen, confirmed) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        assert_eq!(
            peer::read(&mut socket).await.body,
            Some(Message::BranchRegisterRequest(expected))
        );
        drop(socket);
        let mut replacement = registered(&listener).await;
        seen.send(()).unwrap();
        peer::eof(&mut replacement).await;
    });
    let error = rm
        .client()
        .branch_register(original.clone())
        .await
        .unwrap_err();
    assert!(error.is_unknown_outcome());
    match error {
        RmCallError::UnknownOutcome { request, .. } => {
            assert_eq!(*request, Message::BranchRegisterRequest(original))
        }
        other => panic!("{other:?}"),
    };
    tokio::time::timeout(Duration::from_secs(2), confirmed)
        .await
        .unwrap()
        .unwrap();
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn wrong_response_family_is_protocol_uncertainty_and_not_definite_rejection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let value = peer::read(&mut socket).await;
        peer::write(&mut socket, peer::reply(value.id, peer::status_result())).await;
    });
    let error = rm.client().branch_register(request()).await.unwrap_err();
    assert!(matches!(
        error,
        RmCallError::ProtocolFailure {
            may_have_effect: true,
            ..
        }
    ));
    assert!(error.is_unknown_outcome());
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn ambiguous_empty_optional_input_is_rejected_before_the_business_frame_and_closed_clients_stay_closed(
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let client = rm.client();
    let mut bad = request();
    bad.application_data = Some(String::new());
    assert!(matches!(
        client.branch_register(bad).await,
        Err(RmCallError::Invalid { .. })
    ));
    rm.shutdown().await.unwrap();
    assert!(matches!(
        client.branch_register(request()).await,
        Err(RmCallError::LocalFailureBeforeSend { .. })
    ));
}
