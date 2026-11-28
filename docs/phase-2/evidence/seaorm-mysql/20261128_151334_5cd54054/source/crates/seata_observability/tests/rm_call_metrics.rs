//! 认证 HTTP 导出 RM 出站结果，保留克隆在管理器关闭后的本地计数。
#![allow(clippy::unwrap_used)]
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod fixture;
use fixture::{manager, peer, registered};
use seata_observability::{MetricsExporter, MetricsServerConfig, MetricsSources};
use seata_protocol::transaction::{GlobalLockQueryRequest, GlobalLockQueryResponse};
use seata_protocol::{BranchType, Message, ResultCode};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
const TOKEN: &str = "rm-call-private-bearer-0123456789";

fn query() -> GlobalLockQueryRequest {
    GlobalLockQueryRequest {
        xid: Some("tc:rm-call-private-id".into()),
        branch_type: BranchType::At,
        resource_id: Some("mysql://exact-resource".into()),
        lock_key: Some("private-lock:1".into()),
        application_data: Some("private-application-credential".into()),
    }
}
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
fn body(value: &str) -> &str {
    value.split_once("\r\n\r\n").unwrap().1
}

#[tokio::test]
async fn authenticated_rm_call_only_source_has_24_fixed_series_and_shared_clone_counts() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let client = rm.client();
    let (seen_tx, seen) = oneshot::channel();
    let (release, close) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        for failed in [false, true] {
            let request = peer::read(&mut socket).await;
            let mut response = peer::success();
            if failed {
                response.result_code = ResultCode::Failed;
                response.msg = Some("private-tc-credential".into());
            }
            peer::write(
                &mut socket,
                peer::reply(
                    request.id,
                    Message::GlobalLockQueryResponse(GlobalLockQueryResponse {
                        response,
                        lockable: false,
                    }),
                ),
            )
            .await;
        }
        let _ = peer::read(&mut socket).await;
        seen_tx.send(()).unwrap();
        close.await.unwrap();
        drop(socket);
    });
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::default().with_rm_calls(rm.call_metrics_sampler()),
    )
    .await
    .unwrap();
    assert!(fetch(exporter.local_addr(), "wrong-rm-call-token")
        .await
        .starts_with("HTTP/1.1 401"));
    assert!(!client.lock_query(query()).await.unwrap().lockable);
    assert!(matches!(
        client.lock_query(query()).await,
        Err(seata_rm::RmCallError::DefiniteRejected { .. })
    ));
    let mut call = Box::pin(client.lock_query(query()));
    tokio::select! {result=&mut call=>panic!("{result:?}"),_=seen=>{}}
    let active = fetch(exporter.local_addr(), TOKEN).await;
    assert!(body(&active).contains("seata_rm_calls_active{operation=\"lock_query\"} 1"));
    release.send(()).unwrap();
    assert!(call.await.unwrap_err().is_unknown_outcome());
    rm.shutdown().await.unwrap();
    server.await.unwrap();
    assert!(matches!(
        client.lock_query(query()).await,
        Err(seata_rm::RmCallError::LocalFailureBeforeSend { .. })
    ));
    let mut invalid = query();
    invalid.xid = None;
    assert!(matches!(
        client.lock_query(invalid).await,
        Err(seata_rm::RmCallError::Invalid { .. })
    ));
    drop(client);
    let final_response = fetch(exporter.local_addr(), TOKEN).await;
    let text = body(&final_response);
    assert_eq!(text.lines().count(), 24);
    assert!(text.contains("seata_rm_calls_active{operation=\"lock_query\"} 0"));
    for outcome in [
        "observed",
        "tc_rejected",
        "unknown",
        "not_sent",
        "local_rejected",
    ] {
        assert!(text.contains(&format!(
            "seata_rm_calls_total{{operation=\"lock_query\",outcome=\"{outcome}\"}} 1"
        )));
    }
    assert!(text.contains("seata_rm_calls_total{operation=\"lock_query\",outcome=\"cancelled\"} 0"));
    for secret in [
        TOKEN,
        "wrong-rm-call-token",
        "tc:rm-call-private-id",
        "mysql://exact-resource",
        "private-lock:1",
        "private-application-credential",
        "private-tc-credential",
    ] {
        assert!(!text.contains(secret));
    }
    exporter.shutdown().await.unwrap();
}
