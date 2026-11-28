//! 统一 TLS 策略的实际双向握手及错误信任边界测试。
#![cfg(feature = "tls")]
#![allow(clippy::unwrap_used)]
mod support;
#[path = "../../seata_transport/tests/support/tls_peer.rs"]
mod tls_peer;
use seata_client::SeataClient;
use seata_protocol::protocol::{RegisterRMResponse, RegisterTMResponse};
use seata_protocol::Message;
use seata_rm::ResourceCatalog;
use std::time::Duration;
use tokio::net::TcpListener;

#[tokio::test]
async fn both_roles_use_same_mutual_tls_identity_and_peer_binding() {
    let pki = tls_peer::Pki::new();
    let tls = pki.client("seata.example.test");
    let binding = *tls.peer_trust_binding();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (path, store) = support::journal().await;
    let client = SeataClient::builder(
        support::connection(listener.local_addr().unwrap()),
        support::identity(),
    )
    .enable_tm()
    .enable_rm(ResourceCatalog::new(4, 4096).unwrap(), store)
    .transport_tls(tls)
    .build()
    .await
    .unwrap();
    assert_eq!(
        client.transaction_manager().unwrap().peer_trust_binding(),
        Some(&binding)
    );
    let acceptor = pki.acceptor();
    let mut roles = (false, false);
    let mut streams = Vec::new();
    for _ in 0..2 {
        let (socket, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let mut stream = tokio::time::timeout(Duration::from_secs(3), acceptor.accept(socket))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            stream.get_ref().1.peer_certificates().unwrap()[0].as_ref(),
            pki.client_certificate
        );
        let frame = tls_peer::read(&mut stream).await;
        let response = match frame.body.unwrap() {
            Message::RegisterTMRequest(r) => {
                assert!(!roles.0);
                roles.0 = true;
                assert_eq!(r.extra_data, "credential-secret");
                Message::RegisterTMResponse(RegisterTMResponse {
                    identified: true,
                    wire_version: None,
                })
            }
            Message::RegisterRMRequest(r) => {
                assert!(!roles.1);
                roles.1 = true;
                assert_eq!(r.extra_data.as_deref(), Some("credential-secret"));
                Message::RegisterRMResponse(RegisterRMResponse {
                    identified: true,
                    wire_version: None,
                })
            }
            other => panic!("unexpected TLS business frame: {other:?}"),
        };
        tls_peer::write(&mut stream, support::peer::reply(frame.id, response)).await;
        streams.push(stream);
    }
    assert_eq!(roles, (true, true));
    client.shutdown().await.unwrap();
    drop(streams);
    drop(
        seata_rm::FileBranchStore::open(path.clone(), support::options())
            .await
            .unwrap(),
    );
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn wrong_peer_name_sends_no_native_registration_and_no_plaintext_fallback() {
    let pki = tls_peer::Pki::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (path, store) = support::journal().await;
    let client = SeataClient::builder(
        support::connection(listener.local_addr().unwrap()),
        support::identity(),
    )
    .enable_tm()
    .enable_rm(ResourceCatalog::new(4, 4096).unwrap(), store)
    .transport_tls(pki.client("wrong.example.test"))
    .build()
    .await
    .unwrap();
    let acceptor = pki.acceptor();
    for _ in 0..2 {
        let (socket, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
            .await
            .unwrap()
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(3), acceptor.accept(socket))
                .await
                .unwrap()
                .is_err()
        );
    }
    assert_eq!(
        client
            .transaction_manager()
            .unwrap()
            .connection_sampler()
            .snapshot()
            .ready(),
        0
    );
    assert_eq!(
        client
            .resource_manager()
            .unwrap()
            .connection_snapshot()
            .ready(),
        0
    );
    client.shutdown().await.unwrap();
    drop(
        seata_rm::FileBranchStore::open(path.clone(), support::options())
            .await
            .unwrap(),
    );
    std::fs::remove_file(path).unwrap();
}
