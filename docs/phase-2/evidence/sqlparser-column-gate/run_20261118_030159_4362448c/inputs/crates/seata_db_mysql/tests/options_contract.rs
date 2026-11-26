//! 连接配置预算、显式loopback测试TLS选项与凭据脱敏。
#![allow(clippy::unwrap_used)]
use seata_db_mysql::MySqlOptions;
use std::time::Duration;

#[test]
fn options_reject_invalid_identifiers_addresses_and_pool_limits() {
    assert!(MySqlOptions::new("127.0.0.1".into(), 0, "u".into(), "p".into(), "db".into()).is_err());
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
