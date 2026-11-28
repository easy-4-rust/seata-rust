//! 有界日志只读计数通过原认证端点导出，不保留或消费日志队列。
#![cfg(feature = "structured_logs")]
#![allow(clippy::unwrap_used)]
#[path = "../../seata_tm/tests/support/tm_peer.rs"]
mod fixture;
use seata_observability::{
    MetricsExporter, MetricsServerConfig, MetricsSources, StructuredLogConfig,
    StructuredLogSubscriber,
};
use std::{collections::BTreeMap, net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
const TOKEN: &str = "private-log-metrics-token-123456789";
async fn fetch(address: SocketAddr, token: &str) -> String {
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
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8(bytes).unwrap()
}
fn body(text: &str) -> &str {
    text.split_once("\r\n\r\n").unwrap().1
}
fn samples(text: &str) -> BTreeMap<String, u64> {
    body(text)
        .lines()
        .filter(|l| l.starts_with("seata_structured_logs_"))
        .map(|line| {
            let (name, value) = line.rsplit_once(' ').unwrap();
            (name.into(), value.parse().unwrap())
        })
        .collect()
}
fn expected(
    accepted: u64,
    received: u64,
    full: u64,
    closed: u64,
    oversize: u64,
    invalid: u64,
) -> BTreeMap<String, u64> {
    [
        ("seata_structured_logs_accepted_total", accepted),
        ("seata_structured_logs_received_total", received),
        ("seata_structured_logs_dropped_total{reason=\"full\"}", full),
        (
            "seata_structured_logs_dropped_total{reason=\"closed\"}",
            closed,
        ),
        (
            "seata_structured_logs_dropped_total{reason=\"oversize\"}",
            oversize,
        ),
        (
            "seata_structured_logs_dropped_total{reason=\"invalid\"}",
            invalid,
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect()
}
fn emit(xid: &str) {
    tracing::info!(target:"seata_tm::rpc",operation="Status",xid,request_id=-7,outcome="response","tm_rpc_outcome");
}
fn save(name: &str, text: &str) {
    if let Some(folder) = std::env::var_os("SEATA_STRUCTURED_LOG_OUT") {
        let folder = std::path::PathBuf::from(folder);
        assert!(
            folder.is_absolute()
                && (folder.to_string_lossy().starts_with("D:")
                    || folder.to_string_lossy().starts_with("E:"))
        );
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(name), text).unwrap();
    }
}
#[tokio::test]
async fn authenticated_log_only_endpoint_exports_all_six_quiescent_counts_without_identity() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(1, 512).unwrap());
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::default().with_structured_logs(sampler.clone()),
    )
    .await
    .unwrap();
    let unauthorized = fetch(exporter.local_addr(), "wrong-private-token").await;
    assert!(unauthorized.starts_with("HTTP/1.1 401"));
    assert!(!unauthorized.contains("seata_"));
    let zero = fetch(exporter.local_addr(), TOKEN).await;
    assert_eq!(
        samples(&zero),
        expected(0, 0, 0, 0, 0, 0),
        "six structured log metrics missing"
    );
    assert_eq!(body(&zero).lines().count(), 6);
    tracing::subscriber::with_default(subscriber.clone(), || {
        emit("tc:metrics-private-xid");
        emit("tc:metrics-private-xid");
        emit(&"x".repeat(1024));
    });
    let oversize_only = fetch(exporter.local_addr(), TOKEN).await;
    assert_eq!(
        samples(&oversize_only),
        expected(1, 0, 1, 0, 1, 0),
        "oversize-only log counters differ"
    );
    tracing::subscriber::with_default(subscriber.clone(), || {
        tracing::info!(target:"seata_tm::rpc",operation="Status",xid="tc:metrics-private-xid",request_id=-7,outcome="response",password="private-password-probe","tm_rpc_outcome");
    });
    let queued = fetch(exporter.local_addr(), TOKEN).await;
    assert_eq!(
        samples(&queued),
        expected(1, 0, 1, 0, 1, 1),
        "queued log counters differ"
    );
    let line = receiver.recv().await.unwrap();
    assert!(String::from_utf8(line)
        .unwrap()
        .contains("tc:metrics-private-xid"));
    receiver.close();
    tracing::subscriber::with_default(subscriber, || emit("tc:metrics-private-xid"));
    assert!(receiver.recv().await.is_none());
    drop(receiver);
    let final_text = fetch(exporter.local_addr(), TOKEN).await;
    assert_eq!(
        samples(&final_text),
        expected(1, 1, 1, 1, 1, 1),
        "final six log counters differ"
    );
    assert_eq!(body(&final_text).lines().count(), 6);
    assert_eq!(sampler.snapshot().received, 1);
    for secret in [
        TOKEN,
        "wrong-private-token",
        "tc:metrics-private-xid",
        "request_id",
        "private-password-probe",
        "jdbc:mysql",
    ] {
        assert!(!final_text.contains(secret));
    }
    assert_eq!(
        body(&fetch(exporter.local_addr(), TOKEN).await),
        body(&final_text),
        "scrape changed log counters"
    );
    save("log-only-zero.http", &zero);
    save("log-only-queued.http", &queued);
    save("log-only-oversize.http", &oversize_only);
    save("log-only-final.http", &final_text);
    save("log-only-unauthorized.http", &unauthorized);
    exporter.shutdown().await.unwrap();
}
#[tokio::test]
async fn retained_metrics_source_does_not_keep_log_channel_alive() {
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(1, 1024).unwrap());
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::new(None, None).with_structured_logs(sampler.clone()),
    )
    .await
    .unwrap();
    tracing::subscriber::with_default(subscriber, || emit("tc:retained-private-xid"));
    assert!(receiver.recv().await.is_some());
    let closed = tokio::time::timeout(Duration::from_millis(500), receiver.recv()).await;
    assert!(closed.is_ok(), "metrics source retained log sender");
    assert!(closed.unwrap().is_none());
    drop(receiver);
    drop(sampler);
    let text = fetch(exporter.local_addr(), TOKEN).await;
    assert_eq!(samples(&text), expected(1, 1, 0, 0, 0, 0));
    save("retained-final.http", &text);
    exporter.shutdown().await.unwrap();
}
#[tokio::test]
async fn actual_tm_queue_overflow_combines_six_log_and_32_original_tm_series() {
    use seata_protocol::{transaction::GlobalStatusResponse, GlobalStatus, Message};
    let (subscriber, mut receiver, sampler) =
        StructuredLogSubscriber::channel(StructuredLogConfig::new(1, 4096).unwrap());
    let _guard = tracing::subscriber::set_default(subscriber);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = fixture::manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = fixture::registered(&listener).await;
        let mut ids = Vec::new();
        for _ in 0..2 {
            let request = fixture::peer::read(&mut socket).await;
            assert!(matches!(
                request.body,
                Some(Message::GlobalStatusRequest(_))
            ));
            ids.push(request.id);
            fixture::peer::write(
                &mut socket,
                fixture::peer::reply(
                    request.id,
                    Message::GlobalStatusResponse(GlobalStatusResponse {
                        response: fixture::peer::success(),
                        global_status: GlobalStatus::Rollbacking,
                    }),
                ),
            )
            .await;
        }
        fixture::peer::eof(&mut socket).await;
        ids
    });
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::default()
            .with_tm_calls(tm.metrics_sampler())
            .with_structured_logs(sampler.clone()),
    )
    .await
    .unwrap();
    let xid = seata_tm::Xid::parse("tc:tm-metrics-private-xid".into()).unwrap();
    for _ in 0..2 {
        assert_eq!(
            tm.get_status(&xid).await.unwrap().status(),
            GlobalStatus::Rollbacking
        );
    }
    let before = fetch(exporter.local_addr(), TOKEN).await;
    assert_eq!(samples(&before), expected(1, 0, 1, 0, 0, 0));
    assert_eq!(body(&before).lines().count(), 38);
    assert!(before.contains("seata_tm_calls_total{operation=\"status\",outcome=\"observed\"} 2\n"));
    let line = receiver.recv().await.unwrap();
    let event: serde_json::Value = serde_json::from_slice(&line).unwrap();
    receiver.close();
    tm.shutdown().await.unwrap();
    let ids = server.await.unwrap();
    assert_eq!(event["fields"]["request_id"], ids[0].to_string());
    assert_eq!(sampler.snapshot().dropped_full, 1);
    let after = fetch(exporter.local_addr(), TOKEN).await;
    assert_eq!(samples(&after), expected(1, 1, 1, 0, 0, 0));
    assert_eq!(body(&after).lines().count(), 38);
    assert!(!after.contains("tc:tm-metrics-private-xid"));
    save("tm-combined-before.http", &before);
    save("tm-combined-final.http", &after);
    save("tm-actual-facts.json",&serde_json::json!({"actual_request_ids":ids,"first_original_event":event,"actual_observed_calls":2,"one_received_one_dropped":true}).to_string());
    exporter.shutdown().await.unwrap();
}
