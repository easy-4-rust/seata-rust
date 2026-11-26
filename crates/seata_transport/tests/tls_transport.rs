//! Seata 原连接的显式 mTLS 入口验收。
#![cfg(feature = "tls")]
#![allow(clippy::unwrap_used)]
use seata_transport::TlsClientConfig;
#[path = "support/session_peer.rs"]
#[allow(dead_code)]
mod peer;
#[path = "support/tls_peer.rs"]
mod tls_peer;
use seata_transport::{
    CallError, ConnectionPool, RegistrationIdentity, SessionRegistration, TcpConnection,
    TransportError,
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[test]
fn tls_configuration_rejects_empty_trust_and_identity() {
    assert!(TlsClientConfig::new("seata.example.test".into(), vec![], vec![], vec![]).is_err());
    let pki = tls_peer::Pki::new();
    for (name, roots, chain, key) in [
        (
            "bad name",
            vec![pki.root.clone()],
            vec![pki.client_certificate.clone()],
            pki.client_key.clone(),
        ),
        (
            "seata.example.test",
            vec![vec![0; 65537]],
            vec![pki.client_certificate.clone()],
            pki.client_key.clone(),
        ),
        (
            "seata.example.test",
            vec![pki.root.clone(); 33],
            vec![pki.client_certificate.clone()],
            pki.client_key.clone(),
        ),
        (
            "seata.example.test",
            vec![vec![0; 65536]; 32],
            vec![pki.client_certificate.clone()],
            pki.client_key.clone(),
        ),
        (
            "seata.example.test",
            vec![pki.root.clone()],
            vec![vec![1, 2, 3]],
            pki.client_key.clone(),
        ),
        (
            "seata.example.test",
            vec![pki.root.clone()],
            vec![pki.client_certificate.clone()],
            vec![0; 65537],
        ),
    ] {
        assert!(TlsClientConfig::new(name.into(), roots, chain, key).is_err());
    }
    assert_eq!(
        format!("{:?}", pki.client("seata.example.test")),
        "TlsClientConfig { profile: \"TLS1.3 mutual authentication\", .. }"
    );
}

#[tokio::test]
async fn mutually_authenticated_tls_keeps_native_requests_and_original_callback_capability() {
    let pki = tls_peer::Pki::new();
    let tls = pki.client("seata.example.test");
    let acceptor = pki.acceptor();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let expected_client = pki.client_certificate;
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut stream = acceptor.accept(socket).await.unwrap();
        assert_eq!(
            stream.get_ref().1.protocol_version(),
            Some(tokio_rustls::rustls::ProtocolVersion::TLSv1_3)
        );
        assert_eq!(
            stream.get_ref().1.peer_certificates().unwrap()[0].as_ref(),
            expected_client
        );
        let request = tls_peer::read(&mut stream).await;
        tls_peer::write(
            &mut stream,
            peer::reply(request.id, peer::begin_result("tc:tls")),
        )
        .await;
        let mut callback = peer::reply(
            -71,
            seata_protocol::Message::BranchCommitRequest(
                seata_protocol::transaction::BranchCommitRequest {
                    xid: "tc:tls".into(),
                    branch_id: 27,
                    branch_type: seata_protocol::BranchType::At,
                    resource_id: "mysql://tls-resource".into(),
                    application_data: Some("private-tls-data".into()),
                },
            ),
        );
        callback.message_type = seata_protocol::RequestType::RequestSync;
        callback.head_map = vec![("nullable".into(), None)];
        tls_peer::write(&mut stream, callback).await;
        let reply = tls_peer::read(&mut stream).await;
        assert_eq!(reply.id, -71);
        assert_eq!(reply.head_map, vec![("nullable".into(), None)]);
        assert!(
            matches!(reply.body,Some(seata_protocol::Message::BranchCommitResponse(v)) if v.branch_id==27 && v.branch_status==seata_protocol::BranchStatus::PhaseTwoCommitted)
        );
    });
    let mut connection =
        TcpConnection::connect_tls(address, tls_peer::config(Duration::from_secs(2)), &tls)
            .await
            .unwrap();
    let handle = connection.handle();
    assert_eq!(
        peer::xid(
            handle
                .request(peer::begin("private-tls-name"), Duration::from_secs(2))
                .await
                .unwrap()
        ),
        "tc:tls"
    );
    let inbound = connection.next_request().await.unwrap();
    assert_eq!(inbound.connection().generation(), handle.generation());
    inbound
        .respond(
            seata_protocol::Message::BranchCommitResponse(
                seata_protocol::transaction::BranchCommitResponse {
                    response: peer::success(),
                    xid: Some("tc:tls".into()),
                    branch_id: 27,
                    branch_status: seata_protocol::BranchStatus::PhaseTwoCommitted,
                },
            ),
            Duration::from_secs(2),
        )
        .await
        .unwrap();
    server.await.unwrap();
    connection.shutdown().await.unwrap();
}

async fn rejected_server(name: &str, wrong_root: bool) {
    let pki = tls_peer::Pki::new();
    let tls = if wrong_root {
        pki.client_with_roots(name, tls_peer::Pki::new().root)
    } else {
        pki.client(name)
    };
    let acceptor = pki.acceptor();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        assert!(acceptor.accept(socket).await.is_err());
    });
    let error = TcpConnection::connect_tls(address, tls_peer::config(Duration::from_secs(2)), &tls)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        CallError::LocalFailureBeforeSend {
            request_id: None,
            reason: TransportError::TlsHandshakeFailed,
            ..
        }
    ));
    assert!(!error.is_unknown_outcome());
    server.await.unwrap();
}
#[tokio::test]
async fn wrong_ca_rejects_before_native_frames() {
    rejected_server("seata.example.test", true).await;
}
#[tokio::test]
async fn wrong_san_rejects_before_native_frames() {
    rejected_server("other.example.test", false).await;
}

#[tokio::test]
async fn untrusted_client_is_rejected_by_server_without_application_data_acceptance() {
    let pki = tls_peer::Pki::new();
    let other = tls_peer::Pki::new();
    let tls = other.client_with_roots("seata.example.test", pki.root.clone());
    let acceptor = pki.acceptor();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        assert!(acceptor.accept(socket).await.is_err());
    });
    // TLS 1.3 的客户端握手返回可能先于服务端验证客户端最后一轮，不能伪称该错误必然在写前。
    if let Ok(connection) =
        TcpConnection::connect_tls(address, tls_peer::config(Duration::from_secs(2)), &tls).await
    {
        assert!(connection
            .handle()
            .request(peer::begin("must-not-be-accepted"), Duration::from_secs(2))
            .await
            .is_err());
        connection.shutdown().await.unwrap();
    }
    server.await.unwrap();
}

#[tokio::test]
async fn plaintext_peer_never_receives_native_fallback() {
    let pki = tls_peer::Pki::new();
    let tls = pki.client("seata.example.test");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut header = [0; 3];
        socket.read_exact(&mut header).await.unwrap();
        assert_eq!(header[0], 22);
        assert_ne!(header, [0xda, 0xda, 1]);
        socket.write_all(b"not a TLS peer").await.unwrap();
        drop(socket);
        assert!(
            tokio::time::timeout(Duration::from_millis(80), listener.accept())
                .await
                .is_err()
        );
    });
    assert!(matches!(
        TcpConnection::connect_tls(address, tls_peer::config(Duration::from_secs(2)), &tls)
            .await
            .unwrap_err(),
        CallError::LocalFailureBeforeSend {
            request_id: None,
            reason: TransportError::TlsHandshakeFailed,
            ..
        }
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn stalled_handshake_uses_original_connect_deadline_and_pool_shutdown_closes_socket() {
    let pki = tls_peer::Pki::new();
    let tls = pki.client("seata.example.test");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (error, accepted) = tokio::join!(
        TcpConnection::connect_tls(address, tls_peer::config(Duration::from_millis(60)), &tls),
        listener.accept()
    );
    assert!(matches!(
        error.unwrap_err(),
        CallError::LocalFailureBeforeSend {
            request_id: None,
            reason: TransportError::ConnectTimeout,
            ..
        }
    ));
    let (mut socket, _) = accepted.unwrap();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(1), socket.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bytes[0], 22);
    let pool = ConnectionPool::start_tls(tls_peer::pool(address), tls).unwrap();
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut first = [0; 1];
    socket.read_exact(&mut first).await.unwrap();
    assert_eq!(first, [22]);
    tokio::time::timeout(Duration::from_secs(1), pool.shutdown())
        .await
        .unwrap()
        .unwrap();
    let mut rest = Vec::new();
    tokio::time::timeout(Duration::from_secs(1), socket.read_to_end(&mut rest))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn registered_pool_reconnects_with_tls_before_repeating_only_session_registration() {
    let pki = tls_peer::Pki::new();
    let tls = pki.client("seata.example.test");
    let acceptor = pki.acceptor();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut release = Some(release_rx);
        for index in 0..2 {
            let (socket, _) = listener.accept().await.unwrap();
            let mut stream = acceptor.accept(socket).await.unwrap();
            let registration = tls_peer::read(&mut stream).await;
            assert!(matches!(
                registration.body,
                Some(seata_protocol::Message::RegisterTMRequest(_))
            ));
            tls_peer::write(
                &mut stream,
                peer::reply(
                    registration.id,
                    seata_protocol::Message::RegisterTMResponse(
                        seata_protocol::protocol::RegisterTMResponse {
                            identified: true,
                            wire_version: None,
                        },
                    ),
                ),
            )
            .await;
            if index == 0 {
                release.take().unwrap().await.unwrap();
                drop(stream);
            } else {
                let request = tls_peer::read(&mut stream).await;
                tls_peer::write(
                    &mut stream,
                    peer::reply(request.id, peer::begin_result("tc:tls-reconnected")),
                )
                .await;
            }
        }
    });
    let identity =
        RegistrationIdentity::new("1.1.0".into(), "tls-app".into(), "tls-group".into(), None)
            .unwrap();
    let registration = SessionRegistration::tm(identity, Duration::from_secs(2)).unwrap();
    let pool =
        ConnectionPool::start_registered_tls(tls_peer::pool(address), registration, tls).unwrap();
    let handle = pool.handle();
    let first = handle.acquire(Duration::from_secs(2)).await.unwrap();
    release_tx.send(()).unwrap();
    first.closed().await;
    let second = handle.acquire(Duration::from_secs(2)).await.unwrap();
    assert_ne!(first.generation(), second.generation());
    assert_eq!(
        peer::xid(
            second
                .request(peer::begin("single business call"), Duration::from_secs(2))
                .await
                .unwrap()
        ),
        "tc:tls-reconnected"
    );
    server.await.unwrap();
    pool.shutdown().await.unwrap();
}

#[test]
#[ignore = "owned Java TC TLS fixture material exporter"]
fn export_java_tls_material() {
    let folder = std::path::PathBuf::from(std::env::var("SEATA_TRANSPORT_TLS_MATERIAL").unwrap());
    assert!(folder.is_absolute() && folder.starts_with("D:/codex-builds"));
    std::fs::create_dir_all(&folder).unwrap();
    let pki = tls_peer::Pki::new();
    for (name, bytes) in [
        ("root.der", pki.root),
        ("server.der", pki.server_certificate),
        ("server.key.der", pki.server_key),
        ("client.der", pki.client_certificate),
        ("client.key.der", pki.client_key),
    ] {
        std::fs::write(folder.join(name), bytes).unwrap();
    }
}
