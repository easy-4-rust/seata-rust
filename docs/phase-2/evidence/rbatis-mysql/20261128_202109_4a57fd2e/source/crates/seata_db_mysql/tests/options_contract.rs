//! 连接配置预算、显式loopback测试TLS选项与凭据脱敏。
#![allow(clippy::unwrap_used)]
use seata_db_mysql::MySqlOptions;
use std::time::Duration;

#[test]
fn client_identity_requires_bounded_pem_pair_and_never_allows_disabled_tls() {
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "u".into(),
        "p".into(),
        "db".into(),
    )
    .unwrap();
    // 配置测试只验证 PEM 包装，实际 DER 和密钥匹配在现场握手中验证。
    let certificate =
        b"-----BEGIN CERTIFICATE-----\ncGVtLW1hcmtlcg==\n-----END CERTIFICATE-----\n".to_vec();
    let key =
        b"-----BEGIN PRIVATE KEY-----\ncGVtLWtleS1tYXJrZXI=\n-----END PRIVATE KEY-----\n".to_vec();
    let mut maximum_certificate = certificate.clone();
    maximum_certificate.resize(65536, b'\n');
    let mut maximum_key = key.clone();
    maximum_key.resize(65536, b'\n');
    assert!(options
        .clone()
        .with_tls_client_identity_pem(maximum_certificate, maximum_key)
        .is_ok());
    assert!(options.clone().with_tls_client_identity_pem(certificate.clone(),
        b"-----BEGIN ENCRYPTED PRIVATE KEY-----\ncGVtLWtleQ==\n-----END ENCRYPTED PRIVATE KEY-----\n".to_vec()).is_err());
    for (cert, private) in [
        (Vec::new(), key.clone()),
        (certificate.clone(), Vec::new()),
        (vec![b'x'; 65537], key.clone()),
        (certificate.clone(), vec![b'x'; 65537]),
        (b"not-a-certificate".to_vec(), key.clone()),
        (certificate.clone(), b"not-a-key".to_vec()),
    ] {
        assert!(options
            .clone()
            .with_tls_client_identity_pem(cert, private)
            .is_err());
    }
    let secured = options
        .clone()
        .with_tls_client_identity_pem(certificate.clone(), key.clone())
        .unwrap();
    assert!(!format!("{secured:?}").contains("cGVtLWtleS1tYXJrZXI="));
    assert!(!format!("{secured:?}").contains("BEGIN PRIVATE KEY"));
    assert!(secured.with_insecure_loopback_tls().is_err());
    assert!(options
        .with_insecure_loopback_tls()
        .unwrap()
        .with_tls_client_identity_pem(certificate, key)
        .is_err());
}

#[test]
fn options_reject_invalid_identifiers_addresses_and_pool_limits() {
    assert!(MySqlOptions::new("127.0.0.1".into(), 0, "u".into(), "p".into(), "db".into()).is_err());
    for host in ["0.0.0.0", "::"] {
        assert!(
            MySqlOptions::new(host.into(), 3306, "u".into(), "p".into(), "db".into()).is_err(),
            "accepted unspecified MySQL destination {host:?}"
        );
    }
    assert!(MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "   ".into(),
        "p".into(),
        "db".into()
    )
    .is_err());
    assert!(MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "u".into(),
        "p".into(),
        "bad`db".into()
    )
    .is_err());
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "u".into(),
        "p".into(),
        "db".into(),
    )
    .unwrap();
    assert!(options
        .clone()
        .pool_limits(0, Duration::from_secs(1))
        .is_err());
    assert!(options.pool_limits(2, Duration::ZERO).is_err());
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "u".into(),
        "p".into(),
        "db".into(),
    )
    .unwrap();
    assert!(options.operation_timeout(Duration::ZERO).is_err());
}

#[test]
fn disabling_tls_requires_an_explicit_literal_loopback_address() {
    let remote = MySqlOptions::new(
        "database.example.com".into(),
        3306,
        "u".into(),
        "p".into(),
        "db".into(),
    )
    .unwrap();
    assert!(remote.with_insecure_loopback_tls().is_err());
    let local = MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "u".into(),
        "p".into(),
        "db".into(),
    )
    .unwrap();
    assert!(local.with_insecure_loopback_tls().is_ok());
}

#[test]
fn debug_never_prints_password_or_username() {
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "private-user".into(),
        "private-password".into(),
        "db".into(),
    )
    .unwrap();
    let debug = format!("{options:?}");
    assert!(!debug.contains("private-user") && !debug.contains("private-password"));
}

#[test]
fn custom_ca_is_bounded_redacted_and_cannot_be_combined_with_disabled_tls() {
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        3306,
        "u".into(),
        "p".into(),
        "db".into(),
    )
    .unwrap();
    assert!(options.clone().with_tls_ca_pem(Vec::new()).is_err());
    assert!(options
        .clone()
        .with_tls_ca_pem(b"malformed-pem-fixture".to_vec())
        .is_err());
    assert!(options.clone().with_tls_ca_pem(vec![b'x'; 65537]).is_err());
    // 此处只检查配置预算；证书链和主机身份由真实握手用例验证。
    let marker =
        b"-----BEGIN CERTIFICATE-----\ncGVtLW1hcmtlcg==\n-----END CERTIFICATE-----\n".to_vec();
    let mut maximum = marker.clone();
    maximum.resize(65536, b'\n');
    assert!(options.clone().with_tls_ca_pem(maximum).is_ok());
    assert!(options
        .clone()
        .with_tls_ca_pem(
            b"-----BEGIN CERTIFICATE-----\n!invalid-base64!\n-----END CERTIFICATE-----\n".to_vec()
        )
        .is_err());
    let secured = options.clone().with_tls_ca_pem(marker.clone()).unwrap();
    assert!(!format!("{secured:?}").contains("cGVtLW1hcmtlcg=="));
    assert!(secured.with_insecure_loopback_tls().is_err());
    assert!(options
        .with_insecure_loopback_tls()
        .unwrap()
        .with_tls_ca_pem(marker)
        .is_err());
}
