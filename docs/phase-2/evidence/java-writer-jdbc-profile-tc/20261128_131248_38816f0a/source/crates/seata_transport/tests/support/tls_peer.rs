#![allow(clippy::unwrap_used, dead_code)]
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use seata_protocol::{
    protocol::RpcMessage,
    v1::{FrameLimits, ProtocolDecoderV1, ProtocolEncoderV1},
    PayloadLimits,
};
use seata_transport::{ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, TlsClientConfig};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_rustls::{
    rustls::{
        self,
        pki_types::{CertificateDer, PrivatePkcs8KeyDer},
        RootCertStore, ServerConfig,
    },
    TlsAcceptor,
};

pub struct Pki {
    pub root: Vec<u8>,
    pub client_certificate: Vec<u8>,
    pub client_key: Vec<u8>,
    pub server_certificate: Vec<u8>,
    pub server_key: Vec<u8>,
}
impl Pki {
    pub fn new() -> Self {
        let mut params = CertificateParams::default();
        params
            .distinguished_name
            .push(DnType::CommonName, "Seata fixture root CA");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages.push(KeyUsagePurpose::KeyCertSign);
        let ca_key = KeyPair::generate().unwrap();
        let root = params.self_signed(&ca_key).unwrap().der().to_vec();
        let issuer = Issuer::new(params, ca_key);
        let issue = |name: &str, usage| {
            let key = KeyPair::generate().unwrap();
            let mut params = CertificateParams::new(vec![name.into()]).unwrap();
            params.distinguished_name.push(DnType::CommonName, name);
            params.extended_key_usages.push(usage);
            (
                params.signed_by(&key, &issuer).unwrap().der().to_vec(),
                key.serialize_der(),
            )
        };
        let (server_certificate, server_key) =
            issue("seata.example.test", ExtendedKeyUsagePurpose::ServerAuth);
        let (client_certificate, client_key) = issue(
            "seata-client.example.test",
            ExtendedKeyUsagePurpose::ClientAuth,
        );
        Self {
            root,
            client_certificate,
            client_key,
            server_certificate,
            server_key,
        }
    }
    pub fn client(&self, name: &str) -> TlsClientConfig {
        self.client_with_roots(name, self.root.clone())
    }
    pub fn client_with_roots(&self, name: &str, root: Vec<u8>) -> TlsClientConfig {
        TlsClientConfig::new(
            name.into(),
            vec![root],
            vec![self.client_certificate.clone()],
            self.client_key.clone(),
        )
        .unwrap()
    }
    pub fn acceptor(&self) -> TlsAcceptor {
        let mut roots = RootCertStore::empty();
        roots.add(CertificateDer::from(self.root.clone())).unwrap();
        let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
            Arc::new(roots),
            Arc::new(rustls::crypto::ring::default_provider()),
        )
        .build()
        .unwrap();
        let config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .unwrap()
                .with_client_cert_verifier(verifier)
                .with_single_cert(
                    vec![CertificateDer::from(self.server_certificate.clone())],
                    PrivatePkcs8KeyDer::from(self.server_key.clone()).into(),
                )
                .unwrap();
        TlsAcceptor::from(Arc::new(config))
    }
}
pub fn limits() -> FrameLimits {
    FrameLimits::new(
        65536,
        32767,
        128,
        PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
    )
    .unwrap()
}
pub fn config(timeout: Duration) -> ConnectionConfig {
    ConnectionConfig::new(limits(), 8, timeout, Duration::from_secs(2)).unwrap()
}
pub fn pool(address: std::net::SocketAddr) -> ConnectionPoolConfig {
    ConnectionPoolConfig::new(
        address,
        config(Duration::from_secs(2)),
        1,
        8,
        ReconnectPolicy::new(
            Duration::from_millis(20),
            Duration::from_millis(80),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
pub async fn read(stream: &mut (impl AsyncRead + Unpin)) -> RpcMessage {
    let mut header = [0; 16];
    tokio::time::timeout(Duration::from_secs(3), stream.read_exact(&mut header))
        .await
        .unwrap()
        .unwrap();
    let length = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
    assert!((16..=65536).contains(&length));
    let mut bytes = header.to_vec();
    bytes.resize(length, 0);
    stream.read_exact(&mut bytes[16..]).await.unwrap();
    ProtocolDecoderV1::new(limits()).decode(&bytes).unwrap()
}
pub async fn write(stream: &mut (impl AsyncWrite + Unpin), message: RpcMessage) {
    let bytes = ProtocolEncoderV1::new(limits()).encode(&message).unwrap();
    for part in [&bytes[..1], &bytes[1..7], &bytes[7..]] {
        stream.write_all(part).await.unwrap();
        tokio::task::yield_now().await;
    }
}
