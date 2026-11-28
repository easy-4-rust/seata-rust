/// 两个角色共用的显式通道策略，缺省不授权明文。
/// 对应 Java: NettyClientConfig；Rust 统一客户端策略扩展。
#[derive(Debug)]
pub(crate) enum ClientTransportPolicy {
    Plaintext,
    #[cfg(feature = "tls")]
    Tls(seata_transport::TlsClientConfig),
}
