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
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod phase_peer;
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

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn independent_undo_source_exports_20_fixed_series_without_identity_after_close() {
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
    let clone = source.clone();
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::default().with_mysql_undo(source.undo_metrics_sampler()),
    )
    .await
    .unwrap();
    assert!(fetch(exporter.local_addr(), "wrong-token")
        .await
        .starts_with("HTTP/1.1 401"));
    let zero = fetch(exporter.local_addr(), TOKEN).await;
    let zero_body = zero.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(zero_body.lines().count(), 20);
    assert!(zero_body.lines().all(|line| line.ends_with(" 0")));
    let xid = "private-independent-undo-xid";
    assert!(source.fence_undo_log(xid, 77441122).await.is_ok());
    assert!(clone.fence_undo_log(xid, 77441122).await.is_ok());
    assert!(source
        .rollback_undo_log("", 0, MySqlJdbcProfile::Connector8Default)
        .await
        .is_err());
    source.close().await.unwrap();
    drop(source);
    drop(clone);
    let output = fetch(exporter.local_addr(), TOKEN).await;
    assert!(output.starts_with("HTTP/1.1 200"));
    let body = output.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(body.lines().count(), 20);
    assert!(body
        .lines()
        .all(|line| line.starts_with("seata_mysql_undo_")));
    assert!(body.contains("seata_mysql_undo_total{operation=\"fence\",outcome=\"completed\"} 2\n"));
    assert!(body.contains("seata_mysql_undo_observations_total{operation=\"fence\",observation=\"fenced_inserted\"} 1\n"));
    assert!(body.contains("seata_mysql_undo_observations_total{operation=\"fence\",observation=\"fenced_existing\"} 1\n"));
    assert!(body.contains(
        "seata_mysql_undo_total{operation=\"rollback\",outcome=\"before_commit_error\"} 1\n"
    ));
    for secret in [
        TOKEN,
        xid,
        "77441122",
        "jdbc:mysql",
        "seata-mysql-fixture-only",
    ] {
        assert!(!output.contains(secret));
    }
    if let Some(folder) = std::env::var_os("SEATA_UNDO_STORE_OUT") {
        let root = std::path::PathBuf::from(folder);
        std::fs::write(root.join("independent-undo-http-zero.txt"), zero_body).unwrap();
        std::fs::write(root.join("independent-undo-http-after-close.txt"), body).unwrap();
    }
    exporter.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn first_phase_source_exports_31_stage_series_without_promoting_preparation_to_commit() {
    use seata_db_mysql::{MySqlPhaseOneOperation as Operation, MySqlPhaseOneOutcome as Outcome};
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
    let metrics = source.phase_one_metrics_sampler();
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::default().with_mysql_phase_one(manager.phase_one_metrics_sampler()),
    )
    .await
    .unwrap();
    assert!(fetch(exporter.local_addr(), "wrong-token")
        .await
        .starts_with("HTTP/1.1 401"));
    let zero = fetch(exporter.local_addr(), TOKEN).await;
    let zero_body = zero.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(zero_body.lines().count(), 39);
    assert!(zero_body.lines().all(|line| line.ends_with(" 0")));
    // 受控 RM 只提供类型化客户端；非法身份在真实准备入口内拒绝，不发送注册。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = phase_peer::manager(&listener, std::time::Duration::from_secs(2)).await;
    let transaction = source.begin().await.unwrap();
    assert!(manager
        .prepare_transaction(transaction, &rm.client(), "", None)
        .await
        .is_err());
    assert_eq!(
        metrics
            .snapshot(Operation::Prepare)
            .count(Outcome::PrepareFailed),
        1
    );
    let mut transaction = source.begin().await.unwrap();
    let reserved =
        seata_at::SqlRecognizerFactory::recognize("UPDATE seata_rs_at_resource_binding SET v=1")
            .unwrap();
    assert!(transaction.execute(&reserved, &[]).await.is_err());
    transaction.rollback().await.unwrap();
    assert_eq!(
        metrics
            .snapshot(Operation::Execute)
            .count(Outcome::ExecutionFailed),
        1
    );
    assert_eq!(
        metrics
            .snapshot(Operation::Rollback)
            .count(Outcome::RollbackConfirmed),
        1
    );
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
    drop(manager);
    drop(source);
    let output = fetch(exporter.local_addr(), TOKEN).await;
    assert!(output.starts_with("HTTP/1.1 200"));
    let body = output.split_once("\r\n\r\n").unwrap().1;
    assert_eq!(body.lines().count(), 39);
    assert!(body.contains(
        "seata_mysql_phase_one_total{operation=\"prepare\",outcome=\"prepare_failed\"} 1\n"
    ));
    for line in body.lines() {
        assert!(line.starts_with("seata_mysql_phase_one_"));
        assert!(line.ends_with(
            if line.contains("outcome=\"prepare_failed\"")
                || line.contains("outcome=\"execution_failed\"")
                || line.contains("outcome=\"rollback_confirmed\"")
            {
                " 1"
            } else {
                " 0"
            }
        ));
    }
    for secret in [TOKEN, "jdbc:mysql", "seata-mysql-fixture-only"] {
        assert!(!output.contains(secret));
    }
    if let Some(folder) = std::env::var_os("SEATA_UNDO_STORE_OUT") {
        let root = std::path::PathBuf::from(folder);
        std::fs::write(root.join("phase-one-http-zero.txt"), zero_body).unwrap();
        std::fs::write(root.join("phase-one-http-after-close.txt"), body).unwrap();
    }
    exporter.shutdown().await.unwrap();
}
