//! 显式选择原 Java TC 前端 mTLS 代理，默认现场仍使用原明文路径。
pub(super) fn tm(
    builder: seata_tm::TransactionManagerBuilder,
) -> seata_tm::TransactionManagerBuilder {
    #[cfg(feature = "transport_tls")]
    if let Some(tls) = config() {
        return builder.transport_tls(tls);
    }
    #[cfg(not(feature = "transport_tls"))]
    assert!(
        std::env::var_os("SEATA_TRANSPORT_TLS_MATERIAL").is_none(),
        "transport TLS must be compiled explicitly"
    );
    builder
}

pub(super) fn rm(builder: seata_rm::ResourceManagerBuilder) -> seata_rm::ResourceManagerBuilder {
    #[cfg(feature = "transport_tls")]
    if let Some(tls) = config() {
        return builder.transport_tls(tls);
    }
    #[cfg(not(feature = "transport_tls"))]
    assert!(
        std::env::var_os("SEATA_TRANSPORT_TLS_MATERIAL").is_none(),
        "transport TLS must be compiled explicitly"
    );
    builder
}

#[cfg(feature = "transport_tls")]
fn config() -> Option<seata_transport::TlsClientConfig> {
    let folder = std::path::PathBuf::from(std::env::var_os("SEATA_TRANSPORT_TLS_MATERIAL")?);
    let read = |name| std::fs::read(folder.join(name)).unwrap();
    Some(
        seata_transport::TlsClientConfig::new(
            "seata.example.test".into(),
            vec![read("root.der")],
            vec![read("client.der")],
            read("client.key.der"),
        )
        .unwrap(),
    )
}
