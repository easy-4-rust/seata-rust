//! 注册回执必须来自单次真实 TCP 响应，不能由调用方身份冒充。
#![allow(clippy::unwrap_used)]
#[path = "support/rm_peer.rs"]
mod fixture;
use fixture::{manager, peer, registered};
use seata_protocol::transaction::{
    AbstractTransactionResponse, BranchRegisterRequest, BranchRegisterResponse,
};
use seata_protocol::{BranchType, Message, ResultCode, TransactionExceptionCode};
use seata_rm::RmCallError;
use std::time::Duration;
use tokio::net::TcpListener;

fn request(app: Option<&str>) -> BranchRegisterRequest {
    BranchRegisterRequest {
        xid: Some("tc:9007199254740993".into()),
        branch_type: BranchType::At,
        resource_id: Some("mysql://exact-resource".into()),
        lock_key: Some("orders:9007199254740995".into()),
        application_data: app.map(str::to_owned),
    }
}

#[tokio::test]
async fn issued_receipt_keeps_original_identity_optional_fields_and_exact_long() {
    for app in [None, Some("original-private-nonce")] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
        let original = request(app);
        let expected = original.clone();
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
            peer::eof(&mut socket).await;
        });
        let receipt = rm
            .client()
            .branch_register_with_receipt(original)
            .await
            .unwrap();
        assert_eq!(receipt.identity().xid(), "tc:9007199254740993");
        assert_eq!(receipt.identity().branch_id(), 9007199254740995);
        assert_eq!(
            receipt.identity().resource().resource_id(),
            "mysql://exact-resource"
        );
        assert_eq!(receipt.identity().resource().branch_type(), BranchType::At);
        assert_eq!(receipt.identity().application_data(), app);
        assert_eq!(receipt.lock_key(), Some("orders:9007199254740995"));
        assert_eq!(receipt.response().branch_id, 9007199254740995);
        let debug = format!("{receipt:?}");
        for private in [
            "tc:9007199254740993",
            "original-private-nonce",
            "orders:9007199254740995",
            "mysql://exact-resource",
        ] {
            assert!(!debug.contains(private));
        }
        rm.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn rejection_and_invalid_issued_ids_cannot_create_receipts() {
    for (result, id) in [
        (ResultCode::Failed, 42),
        (ResultCode::Success, 0),
        (ResultCode::Success, -1),
    ] {
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
                            result_code: result,
                            msg: if result == ResultCode::Failed {
                                Some("private-failure".into())
                            } else {
                                None
                            },
                            transaction_exception_code:
                                TransactionExceptionCode::BranchRegisterFailed,
                        },
                        branch_id: id,
                    }),
                ),
            )
            .await;
            peer::eof(&mut socket).await;
        });
        let error = rm
            .client()
            .branch_register_with_receipt(request(None))
            .await
            .unwrap_err();
        if result == ResultCode::Failed {
            assert!(matches!(error, RmCallError::DefiniteRejected { .. }));
        } else {
            assert!(error.is_unknown_outcome());
        }
        rm.shutdown().await.unwrap();
        server.await.unwrap();
    }
}

#[tokio::test]
async fn lost_issued_response_has_no_receipt_and_reconnection_does_not_register_again() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_millis(400)).await;
    let original = request(Some("original-private-nonce"));
    let expected = original.clone();
    let (seen, confirmed) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        assert_eq!(
            peer::read(&mut socket).await.body,
            Some(Message::BranchRegisterRequest(expected))
        );
        drop(socket);
        let mut next = registered(&listener).await;
        seen.send(()).unwrap();
        peer::eof(&mut next).await;
    });
    let error = rm
        .client()
        .branch_register_with_receipt(original.clone())
        .await
        .unwrap_err();
    assert!(error.is_unknown_outcome());
    match error {
        RmCallError::UnknownOutcome { request, .. }
        | RmCallError::ProtocolFailure { request, .. } => {
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
