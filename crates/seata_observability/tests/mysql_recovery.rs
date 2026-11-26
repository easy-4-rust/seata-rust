//! 真实数据源控制失败与 Bearer HTTP；mysql 功能显式启用。
#![cfg(feature = "mysql")]
#![allow(clippy::unwrap_used)]
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{MySqlDataSource, MySqlDataSourceManager, MySqlOptions};
use seata_observability::{MetricsExporter, MetricsServerConfig, MetricsSources};
use seata_rm::{BranchIdentity, BranchPhase, BranchResolution, BranchResource};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
const TOKEN: &str = "mysql-private-bearer-0123456789012345";
async fn fetch(address: std::net::SocketAddr, token: &str) -> String {
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket
        .write_all(
            format!(
                "GET /metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut output = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        socket.read_to_end(&mut output),
    )
    .await
    .unwrap()
    .unwrap();
    String::from_utf8(output).unwrap()
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn mysql_only_source_exports_42_fixed_series_after_source_close() {
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
        std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap();
    let source = MySqlDataSource::connect(options).await.unwrap();
    let manager =
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap();
    let identity = BranchIdentity::new(
        "private-mysql-xid".into(),
        42,
        manager.key().clone(),
        Some("private-application-credential".into()),
    )
    .unwrap();
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::default().with_mysql_recovery(manager.recovery_metrics_sampler()),
    )
    .await
    .unwrap();
    let unauthorized = fetch(exporter.local_addr(), "wrong-token").await;
    assert!(unauthorized.starts_with("HTTP/1.1 401"));
    source.close().await.unwrap();
    assert_eq!(
        manager
            .recover(&identity, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown
    );
    drop(manager);
    drop(source);
    let output = fetch(exporter.local_addr(), TOKEN).await;
    let body = output.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(body.lines().count(), 42);
    assert!(body.contains("seata_mysql_control_total{operation=\"bind_rollback\",outcome=\"before_commit_error\"} 1\n"));
    assert!(body.contains(
        "seata_mysql_control_total{operation=\"bind_rollback\",outcome=\"completed\"} 0\n"
    ));
    for line in body.lines() {
        assert!(line.starts_with("seata_mysql_control_"));
    }
    for secret in [
        TOKEN,
        "private-mysql-xid",
        "private-application-credential",
        "jdbc:mysql",
        "seata-mysql-fixture-only",
    ] {
        assert!(!output.contains(secret));
    }
    exporter.shutdown().await.unwrap();
}
