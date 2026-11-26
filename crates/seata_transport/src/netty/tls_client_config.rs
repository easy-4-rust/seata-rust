use crate::TransportError;
use std::{fmt, sync::Arc};
use tokio_rustls::{
    rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer, ServerName},
        ClientConfig, RootCertStore,
    },
    TlsConnector,
};

/// 显式 TLS 1.3 双向认证配置，不提供跳过验证或回退明文的能力。
/// 对应 Java: Netty 客户端 SSL 上下文配置职责；证书认证先于 Seata 登记。
#[derive(Clone)]
pub struct TlsClientConfig {
    connector: TlsConnector,
    server_name: ServerName<'static>,
}

impl TlsClientConfig {
    /// 参数为预期 DNS/IP SAN、DER 根集、客户端 DER 链及 DER 私钥；返回验证后的共享安全配置。
    /// 根集/链各最多 32 项、每项最多 64 KiB，总输入最多 1 MiB；私钥不得为空或超过 64 KiB。
    pub fn new(
        server_name: String,
        roots_der: Vec<Vec<u8>>,
        chain_der: Vec<Vec<u8>>,
        key_der: Vec<u8>,
    ) -> Result<Self, TransportError> {
        let invalid = || TransportError::InvalidConfiguration {
            reason: "bounded TLS 1.3 mutual-authentication material required",
        };
        let valid_chain = |chain: &[Vec<u8>]| {
            !chain.is_empty()
                && chain.len() <= 32
                && chain
                    .iter()
                    .all(|cert| !cert.is_empty() && cert.len() <= 65536)
        };
        if !valid_chain(&roots_der)
            || !valid_chain(&chain_der)
            || key_der.is_empty()
            || key_der.len() > 65536
        {
            return Err(invalid());
        }
        let total = roots_der
            .iter()
            .chain(&chain_der)
            .map(Vec::len)
            .sum::<usize>()
            + key_der.len();
        if total > 1_048_576 || server_name.is_empty() || server_name.len() > 253 {
            return Err(invalid());
        }
        let server_name = ServerName::try_from(server_name).map_err(|_| invalid())?;
        let mut roots = RootCertStore::empty();
        for certificate in roots_der {
            roots
                .add(CertificateDer::from(certificate))
                .map_err(|_| invalid())?;
        }
        let key = PrivateKeyDer::try_from(key_der).map_err(|_| invalid())?;
        let mut config =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|_| invalid())?
                .with_root_certificates(roots)
                .with_client_auth_cert(
                    chain_der.into_iter().map(CertificateDer::from).collect(),
                    key,
                )
                .map_err(|_| invalid())?;
        // 每个新代际都重新验证完整握手，不把旧会话票据当作新的身份审查。
        config.resumption = rustls::client::Resumption::disabled();
        Ok(Self {
            connector: TlsConnector::from(Arc::new(config)),
            server_name,
        })
    }

    /// 参数为尚未发送 V1 字节的原 TCP 流和原绝对期限；返回经验证的同一物理 TLS 流。
    pub(crate) async fn connect(
        &self,
        stream: tokio::net::TcpStream,
        deadline: tokio::time::Instant,
    ) -> Result<tokio_rustls::client::TlsStream<tokio::net::TcpStream>, TransportError> {
        match tokio::time::timeout_at(
            deadline,
            self.connector.connect(self.server_name.clone(), stream),
        )
        .await
        {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(_)) => Err(TransportError::TlsHandshakeFailed),
            Err(_) => Err(TransportError::ConnectTimeout),
        }
    }
}

impl fmt::Debug for TlsClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TlsClientConfig")
            .field("profile", &"TLS1.3 mutual authentication")
            .finish_non_exhaustive()
    }
}
