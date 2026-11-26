//! 本机 HTTP 采集的授权、字段边界和断线累计现场测试。
#![allow(clippy::unwrap_used)]

use seata_observability::{MetricsExporter, MetricsServerConfig, MetricsSources};
use seata_protocol::protocol::RpcMessage;
use seata_protocol::v1::{FrameLimits, ProtocolEncoderV1};
use seata_protocol::{CompressorType, PayloadLimits, RequestType, SerializerType};
use seata_transport::{ConnectionConfig, ConnectionPool, ConnectionPoolConfig, ReconnectPolicy};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "top-secret-metrics-token-1234567890";

async fn fetch(address: SocketAddr, request: &str) -> String {
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8(response).unwrap()
}

fn request(token: &str) -> String {
    format!("GET /metrics HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\n\r\n")
}

fn pool_config(endpoint: SocketAddr) -> ConnectionPoolConfig {
    let limits = FrameLimits::new(
        64 * 1024,
        32767,
        128,
        PayloadLimits::new(64 * 1024, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let connection =
        ConnectionConfig::new(limits, 4, Duration::from_secs(2), Duration::from_secs(2)).unwrap();
    let reconnect = ReconnectPolicy::new(
        Duration::from_secs(1),
        Duration::from_secs(2),
        Duration::from_secs(10),
    )
    .unwrap();
    ConnectionPoolConfig::new(endpoint, connection, 1, 4, reconnect).unwrap()
}

#[test]
fn endpoint_requires_loopback_and_strong_visible_token() {
    assert!(MetricsServerConfig::new("0.0.0.0:0".parse().unwrap(), TOKEN.into()).is_err());
    assert!(MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), "short".into()).is_err());
    assert!(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), format!("{TOKEN}\n")).is_err()
    );
    assert!(MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).is_ok());
}

#[tokio::test]
async fn exporter_rejects_empty_sources_before_binding() {
    let config = MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap();
    assert!(MetricsExporter::start(config, MetricsSources::default())
        .await
        .is_err());
}

#[tokio::test]
async fn endpoint_rejects_missing_wrong_duplicate_and_oversized_authorization() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let pool = ConnectionPool::start(pool_config(listener.local_addr().unwrap())).unwrap();
    let config = MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap();
    let exporter = MetricsExporter::start(
        config,
        MetricsSources::new(Some(pool.handle().sampler()), None),
    )
    .await
    .unwrap();
    let address = exporter.local_addr();
    for request in [
        "GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n".to_owned(),
        request("wrong-secret-12345678901234567890"),
        format!(
            "GET /metrics HTTP/1.1\r\nAuthorization: Bearer {TOKEN}\r\nAuthorization: Bearer {TOKEN}\r\n\r\n"
        ),
    ] {
        let response = fetch(address, &request).await;
        assert!(response.starts_with("HTTP/1.1 401 Unauthorized"));
        assert!(!response.contains(TOKEN));
    }
    let prefix = "GET /metrics HTTP/1.1\r\nX-Fill: ";
    let oversized = format!("{prefix}{}", "a".repeat(4096 - prefix.len()));
    let response = fetch(address, &oversized).await;
    assert!(response.starts_with("HTTP/1.1 431 Request Header Fields Too Large"));
    assert!(!response.contains(TOKEN));
    let mut partial = TcpStream::connect(address).await.unwrap();
    partial
        .write_all(b"GET /metrics HTTP/1.1\r\n")
        .await
        .unwrap();
    let mut byte = [0u8; 1];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), partial.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0,
        "unfinished headers must time out and close"
    );
    exporter.shutdown().await.unwrap();
    pool.shutdown().await.unwrap();
}

#[tokio::test]
async fn authorized_pull_reports_retired_connection_without_sensitive_labels() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let limits = FrameLimits::new(
        64 * 1024,
        32767,
        128,
        PayloadLimits::new(64 * 1024, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let pool = ConnectionPool::start(pool_config(endpoint)).unwrap();
    let sampler = pool.handle().sampler();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let heartbeat = RpcMessage {
            id: 7,
            message_type: RequestType::HeartbeatRequest,
            codec: SerializerType::Seata,
            compressor: CompressorType::None,
            head_map: vec![],
            body: None,
        };
        let bytes = ProtocolEncoderV1::new(limits).encode(&heartbeat).unwrap();
        socket.write_all(&bytes).await.unwrap();
        tokio::time::sleep(Duration::from_millis(40)).await;
    });
    server.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = sampler.snapshot();
            if snapshot.cumulative.heartbeats_received == 1
                && snapshot.slots[0].connection.is_none()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let config = MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap();
    let exporter = MetricsExporter::start(
        config,
        MetricsSources::new(Some(sampler.clone()), Some(sampler)),
    )
    .await
    .unwrap();
    let response = fetch(exporter.local_addr(), &request(TOKEN)).await;
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("seata_transport_heartbeats_received_total{role=\"tm\"} 1\n"));
    assert!(response.contains("seata_transport_heartbeats_received_total{role=\"rm\"} 1\n"));
    assert!(response.contains("seata_transport_pending_requests{role=\"tm\",slot=\"0\"} 0\n"));
    assert!(response
        .contains("seata_transport_slot_state{role=\"tm\",slot=\"0\",state=\"backoff\"} 1\n"));
    for secret in [
        TOKEN,
        "xid",
        "branch_id",
        "resource_url",
        "password",
        "127.0.0.1:",
    ] {
        assert!(!response.contains(secret), "unexpected secret: {secret}");
    }
    exporter.shutdown().await.unwrap();
    pool.shutdown().await.unwrap();
}
