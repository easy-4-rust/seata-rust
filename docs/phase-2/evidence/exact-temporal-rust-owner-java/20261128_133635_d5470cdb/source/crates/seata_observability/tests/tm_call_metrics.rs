//! 认证 HTTP 导出真实 TM 结果计数，不含业务身份或响应内容。
#![allow(clippy::unwrap_used)]
#[path = "../../seata_tm/tests/support/tm_peer.rs"]
mod fixture;
use fixture::{manager, peer, registered};
use seata_observability::{MetricsExporter, MetricsServerConfig, MetricsSources};
use seata_protocol::{GlobalStatus, Message, ResultCode};
use seata_tm::{TmError, Xid};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

const TOKEN: &str = "private-metrics-bearer-value-123456789";

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
    let mut output = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut output))
        .await
        .unwrap()
        .unwrap();
    String::from_utf8(output).unwrap()
}

#[tokio::test]
async fn authenticated_endpoint_exports_bounded_tm_results_and_final_counts() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let (seen_tx, seen) = oneshot::channel();
    let (close, release) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        for rejected in [false, true] {
            let request = peer::read(&mut socket).await;
            let mut response = peer::success();
            if rejected {
                response.result_code = ResultCode::Failed;
            }
            if rejected {
                response.msg = Some("credential-sensitive-tc-message".into());
            }
            peer::write(
                &mut socket,
                peer::reply(
                    request.id,
                    Message::GlobalStatusResponse(
                        seata_protocol::transaction::GlobalStatusResponse {
                            response,
                            global_status: GlobalStatus::Rollbacking,
                        },
                    ),
                ),
            )
            .await;
        }
        let _ = peer::read(&mut socket).await;
        seen_tx.send(()).unwrap();
        release.await.unwrap();
        drop(socket);
    });
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::default().with_tm_calls(tm.metrics_sampler()),
    )
    .await
    .unwrap();
    assert!(fetch(exporter.local_addr(), "wrong-bearer-value")
        .await
        .starts_with("HTTP/1.1 401"));
    let xid = Xid::parse("tc:private-identity-sentinel".into()).unwrap();
    let observed = tm.get_status(&xid).await.unwrap();
    assert_eq!(observed.status(), GlobalStatus::Rollbacking);
    assert!(matches!(
        tm.get_status(&xid).await,
        Err(TmError::DefiniteRejected { .. })
    ));
    let mut pending = Box::pin(tm.get_status(&xid));
    tokio::select! {
        result = &mut pending => panic!("unexpected early completion: {result:?}"),
        _ = seen => {}
    }
    let active = fetch(exporter.local_addr(), TOKEN).await;
    assert!(active.contains("seata_tm_calls_active{operation=\"status\"} 1"));
    close.send(()).unwrap();
    assert!(pending.await.unwrap_err().is_unknown_outcome());
    let before_shutdown = fetch(exporter.local_addr(), TOKEN).await;
    tm.shutdown().await.unwrap();
    server.await.unwrap();
    let after_shutdown = fetch(exporter.local_addr(), TOKEN).await;
    let body = |response: &str| response.split_once("\r\n\r\n").unwrap().1.to_owned();
    assert_eq!(body(&after_shutdown), body(&before_shutdown));
    let text = body(&after_shutdown);
    assert_eq!(text.lines().count(), 32);
    assert!(text.contains("seata_tm_calls_active{operation=\"status\"} 0"));
    for outcome in ["observed", "tc_rejected", "unknown"] {
        assert!(text.contains(&format!(
            "seata_tm_calls_total{{operation=\"status\",outcome=\"{outcome}\"}} 1"
        )));
    }
    for outcome in ["not_sent", "local_rejected", "cancelled"] {
        assert!(text.contains(&format!(
            "seata_tm_calls_total{{operation=\"status\",outcome=\"{outcome}\"}} 0"
        )));
    }
    for secret in [
        TOKEN,
        "wrong-bearer-value",
        "private-identity-sentinel",
        "credential-sensitive-tc-message",
    ] {
        assert!(!text.contains(secret));
    }
    exporter.shutdown().await.unwrap();
}
