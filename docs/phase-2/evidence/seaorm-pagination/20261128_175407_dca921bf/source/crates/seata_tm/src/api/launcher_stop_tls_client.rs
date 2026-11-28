use super::{
    launcher_stop_tls::{config_error, timeout_error, validate_trust, verify_peer, STOP_ALPN},
    LauncherStopChallenge, LauncherStopExchange, LauncherStopVerifier,
};
use rustls::{
    client::Resumption,
    pki_types::{CertificateDer, PrivateKeyDer, ServerName},
    ClientConfig, RootCertStore,
};
use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;

/// 停止证明的 TLS 接管方，验证签发主机证书链、主机名和指纹。对应 Java 无直接类型。
pub struct LauncherStopTlsClient {
    connector: TlsConnector,
    server_name: ServerName<'static>,
    pins: Vec<[u8; 32]>,
}

impl LauncherStopTlsClient {
    /// 使用预配签发主机名、CA、接管方证书链和私钥，以及允许的签发方证书 SHA-256 指纹构建连接器。
    pub fn new(
        server_name: ServerName<'static>,
        roots: RootCertStore,
        chain: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
        pins: Vec<[u8; 32]>,
    ) -> io::Result<Self> {
        validate_trust(&roots, &pins)?;
        let mut config =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(config_error)?
                .with_root_certificates(roots)
                .with_client_auth_cert(chain, key)
                .map_err(config_error)?;
        config.alpn_protocols = vec![STOP_ALPN.to_vec()];
        config.resumption = Resumption::disabled();
        Ok(Self {
            connector: TlsConnector::from(Arc::new(config)),
            server_name,
            pins,
        })
    }

    /// 在已连接的流上执行双向 TLS 与单次签名请求，返回经签名验证的结果。
    /// 同一预算覆盖握手与交换，同时受挑战剩余时间限制。
    pub async fn request(
        &self,
        stream: impl AsyncRead + AsyncWrite + Unpin,
        verifier: &LauncherStopVerifier,
        challenge: &LauncherStopChallenge,
        budget: Duration,
    ) -> io::Result<[u8; 64]> {
        LauncherStopExchange::check_budget(budget)?;
        if !verifier.accepts_challenge(challenge) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "invalid TM stop challenge",
            ));
        }
        let deadline_budget =
            budget.min(challenge.deadline.saturating_duration_since(Instant::now()));
        tokio::time::timeout(deadline_budget, async {
            let mut stream = self
                .connector
                .connect(self.server_name.clone(), stream)
                .await?;
            let (_, session) = stream.get_ref();
            verify_peer(
                session.alpn_protocol(),
                session.peer_certificates(),
                &self.pins,
            )?;
            LauncherStopExchange::request(&mut stream, verifier, challenge, budget).await
        })
        .await
        .map_err(|_| timeout_error())?
    }
}
