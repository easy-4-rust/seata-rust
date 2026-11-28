//! 停止证明只在双向认证、证书指纹匹配的 TLS 连接上交换。
#![allow(clippy::unwrap_used)]
use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use ring::{
    digest::{digest, SHA256},
    rand::SystemRandom,
    signature::Ed25519KeyPair,
};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    RootCertStore,
};
use seata_tm::{
    LauncherProcessMonitor, LauncherStopSigner, LauncherStopTlsClient, LauncherStopTlsServer,
    LauncherStopVerifier,
};
use std::{io::Write, process::Command, time::Duration};
use tokio::net::{TcpListener, TcpStream};

#[path = "support/stop_attestation_node.rs"]
mod node;
#[path = "support/tm_peer.rs"]
mod node_peer;

const OWNER: [u8; 32] = [0x72; 32];
const SCOPE: [u8; 32] = [0x45; 32];
const BUDGET: Duration = Duration::from_secs(3);

#[test]
#[ignore = "child of the TLS stop proof fixture"]
fn tls_stopped_worker_fixture() {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(b"OWNER_INSTANCE=").unwrap();
    for byte in OWNER {
        write!(stdout, "{byte:02x}").unwrap();
    }
    stdout.write_all(b"\n").unwrap();
    stdout.flush().unwrap();
    std::process::exit(0);
}

struct Identity {
    leaf: CertificateDer<'static>,
    pkcs8: Vec<u8>,
}

impl Identity {
    fn issue(issuer: &Issuer<'_, KeyPair>, domain: &str, usage: ExtendedKeyUsagePurpose) -> Self {
        let mut params = CertificateParams::new(vec![domain.into()]).unwrap();
        params.extended_key_usages.push(usage);
        let key = KeyPair::generate().unwrap();
        Self {
            leaf: params.signed_by(&key, issuer).unwrap().der().clone(),
            pkcs8: key.serialize_der(),
        }
    }

    fn key(&self) -> PrivateKeyDer<'static> {
        PrivatePkcs8KeyDer::from(self.pkcs8.clone()).into()
    }

    fn pin(&self) -> [u8; 32] {
        digest(&SHA256, self.leaf.as_ref())
            .as_ref()
            .try_into()
            .unwrap()
    }
}

#[tokio::test]
async fn requires_mutual_tls_and_pinned_peer_before_exchanging_stop_proof() {
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    let ca_key = KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let issuer = Issuer::new(ca_params, ca_key);
    let mut roots = RootCertStore::empty();
    roots.add(ca_cert.der().clone()).unwrap();
    let server_identity = Identity::issue(
        &issuer,
        "signer.example.test",
        ExtendedKeyUsagePurpose::ServerAuth,
    );
    let recovery = Identity::issue(
        &issuer,
        "recovery.example.test",
        ExtendedKeyUsagePurpose::ClientAuth,
    );
    let other_recovery = Identity::issue(
        &issuer,
        "other.example.test",
        ExtendedKeyUsagePurpose::ClientAuth,
    );
    let server = LauncherStopTlsServer::new(
        roots.clone(),
        vec![server_identity.leaf.clone()],
        server_identity.key(),
        vec![recovery.pin()],
    )
    .unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--ignored", "--exact", "tls_stopped_worker_fixture"]);
    let mut monitor = LauncherProcessMonitor::spawn(&mut command).unwrap();
    let proof = tokio::time::timeout(BUDGET, async {
        loop {
            if let Some(proof) = monitor.try_confirm_exit().unwrap() {
                break proof;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
    let signer = LauncherStopSigner::from_pkcs8(key.as_ref(), SCOPE).unwrap();
    let (verifier, mut rotation) = LauncherStopVerifier::new_rotatable(signer.public_key(), SCOPE);
    let replacement = [0x21; 32];
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    for (identity, domain, pin, accepts) in [
        (
            &recovery,
            "signer.example.test",
            server_identity.pin(),
            true,
        ),
        (
            &other_recovery,
            "signer.example.test",
            server_identity.pin(),
            false,
        ),
        (
            &recovery,
            "wrong.example.test",
            server_identity.pin(),
            false,
        ),
        (&recovery, "signer.example.test", [0; 32], false),
    ] {
        let client = LauncherStopTlsClient::new(
            domain.to_owned().try_into().unwrap(),
            roots.clone(),
            vec![identity.leaf.clone()],
            identity.key(),
            vec![pin],
        )
        .unwrap();
        let challenge = verifier.challenge(&OWNER, &replacement, BUDGET).unwrap();
        let socket = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (listener_socket, _) = listener.accept().await.unwrap();
        let (received, served) = tokio::join!(
            client.request(socket, &verifier, &challenge, BUDGET),
            server.respond(listener_socket, &monitor, &proof, &signer, BUDGET),
        );
        if accepts {
            assert!(verifier.verifies(&challenge, &received.unwrap(), &OWNER, &replacement));
            served.unwrap();
        } else {
            assert!(received.is_err() && served.is_err());
        }
    }

    let new_key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
    let new_signer = LauncherStopSigner::from_pkcs8(new_key.as_ref(), SCOPE).unwrap();
    let old_public = signer.public_key();
    let new_public = new_signer.public_key();
    let client = LauncherStopTlsClient::new(
        "signer.example.test".try_into().unwrap(),
        roots.clone(),
        vec![recovery.leaf.clone()],
        recovery.key(),
        vec![server_identity.pin()],
    )
    .unwrap();
    let in_flight = verifier.challenge(&OWNER, &replacement, BUDGET).unwrap();
    rotation.replace(&[old_public, new_public]).unwrap();
    assert!(rotation.replace(&[[0; 32]]).is_err());
    for active in [&signer, &new_signer] {
        let socket = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (listener_socket, _) = listener.accept().await.unwrap();
        let (received, served) = tokio::join!(
            client.request(socket, &verifier, &in_flight, BUDGET),
            server.respond(listener_socket, &monitor, &proof, active, BUDGET),
        );
        assert!(verifier.verifies(&in_flight, &received.unwrap(), &OWNER, &replacement));
        served.unwrap();
    }
    assert!(rotation.replace(&[]).is_err());
    assert!(rotation.replace(&[new_public, new_public]).is_err());
    rotation.replace(&[new_public]).unwrap();
    for (active, accepted) in [(&signer, false), (&new_signer, true)] {
        let socket = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (listener_socket, _) = listener.accept().await.unwrap();
        let (received, served) = tokio::join!(
            client.request(socket, &verifier, &in_flight, BUDGET),
            server.respond(listener_socket, &monitor, &proof, active, BUDGET),
        );
        served.unwrap();
        if accepted {
            assert!(verifier.verifies(&in_flight, &received.unwrap(), &OWNER, &replacement));
        } else {
            assert!(
                matches!(received, Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied)
            );
        }
    }

    let mut anonymous = rustls::ClientConfig::builder_with_provider(
        rustls::crypto::ring::default_provider().into(),
    )
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    anonymous.alpn_protocols = vec![b"seata-tm-stop/1".to_vec()];
    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(anonymous));
    let socket = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (listener_socket, _) = listener.accept().await.unwrap();
    let (_, denied) = tokio::join!(
        connector.connect("signer.example.test".try_into().unwrap(), socket),
        server.respond(listener_socket, &monitor, &proof, &signer, BUDGET),
    );
    assert!(matches!(denied, Err(error) if error.kind() == std::io::ErrorKind::InvalidData));

    let _idle = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (listener_socket, _) = listener.accept().await.unwrap();
    let stalled = server
        .respond(
            listener_socket,
            &monitor,
            &proof,
            &signer,
            Duration::from_millis(20),
        )
        .await;
    assert!(matches!(stalled, Err(error) if error.kind() == std::io::ErrorKind::TimedOut));
}
