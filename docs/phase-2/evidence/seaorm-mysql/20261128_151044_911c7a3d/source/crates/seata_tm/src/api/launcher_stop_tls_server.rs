use super::{
    launcher_stop_tls::{config_error, timeout_error, validate_trust, verify_peer, STOP_ALPN},
    LauncherProcessMonitor, LauncherStopExchange, LauncherStopSigner, StoppedLauncherOwner,
};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    server::WebPkiClientVerifier,
    RootCertStore, ServerConfig,
};
use std::{io, sync::Arc, time::Duration};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsAcceptor;

/// 停止证明的 TLS 签发方，要求接管方通过证书链和指纹核验。对应 Java 无直接类型。
pub struct LauncherStopTlsServer {
    acceptor: TlsAcceptor,
    pins: Vec<[u8; 32]>,
}

impl LauncherStopTlsServer {
    /// 使用预配 CA、签发方证书链和私钥及允许的接管方证书 SHA-256 指纹构建签发端。
    pub fn new(
        roots: RootCertStore,
        chain: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
        pins: Vec<[u8; 32]>,
    ) -> io::Result<Self> {
        validate_trust(&roots, &pins)?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let client_auth =
            WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider.clone())
                .build()
                .map_err(config_error)?;
        let mut config = ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(config_error)?
            .with_client_cert_verifier(client_auth)
            .with_single_cert(chain, key)
            .map_err(config_error)?;
        config.alpn_protocols = vec![STOP_ALPN.to_vec()];
        config.send_tls13_tickets = 0;
        Ok(Self {
            acceptor: TlsAcceptor::from(Arc::new(config)),
            pins,
        })
    }

    /// 在已接纳的流上要求双向 TLS，通过核验后处理一次签名请求。
    /// 同一预算覆盖握手及应答；签名仍要求原监护器确认退出和实例匹配。
    pub async fn respond(
        &self,
        stream: impl AsyncRead + AsyncWrite + Unpin,
        monitor: &LauncherProcessMonitor,
        proof: &StoppedLauncherOwner,
        signer: &LauncherStopSigner,
        budget: Duration,
    ) -> io::Result<()> {
        LauncherStopExchange::check_budget(budget)?;
        tokio::time::timeout(budget, async {
            let mut stream = self.acceptor.accept(stream).await?;
            let (_, session) = stream.get_ref();
            verify_peer(
                session.alpn_protocol(),
                session.peer_certificates(),
                &self.pins,
            )?;
            LauncherStopExchange::respond(&mut stream, monitor, proof, signer, budget).await
        })
        .await
        .map_err(|_| timeout_error())?
    }
}
