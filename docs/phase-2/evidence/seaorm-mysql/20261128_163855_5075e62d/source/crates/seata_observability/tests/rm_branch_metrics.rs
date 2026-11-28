//! 真实原连接回调及认证 HTTP 导出的 RM 日志 gauge 和分派 counter。
#![allow(clippy::unwrap_used)]
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod fixture;
use fixture::{peer, registered};
use seata_observability::{MetricsExporter, MetricsServerConfig, MetricsSources};
use seata_protocol::transaction::{BranchCommitRequest, BranchRollbackRequest};
use seata_protocol::{BranchType, Message, RequestType, ResultCode};
use seata_rm::{ResourceCatalog, ResourceManager};
use std::{
    net::SocketAddr,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
const TOKEN: &str = "rm-private-metrics-bearer-0123456789";

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
fn body(response: &str) -> &str {
    response.split_once("\r\n\r\n").unwrap().1
}

async fn send_callback(socket: &mut TcpStream, wire: i32, commit: bool) {
    let body = if commit {
        Message::BranchCommitRequest(BranchCommitRequest {
            xid: "tc:rm-private-identity".into(),
            branch_id: 123456789,
            branch_type: BranchType::At,
            resource_id: "mysql://exact-resource".into(),
            application_data: Some("private-application-credential".into()),
        })
    } else {
        Message::BranchRollbackRequest(BranchRollbackRequest {
            xid: "tc:rm-private-identity".into(),
            branch_id: 123456789,
            branch_type: BranchType::At,
            resource_id: "mysql://exact-resource".into(),
            application_data: Some("private-application-credential".into()),
        })
    };
    let mut request = peer::reply(wire, body);
    request.message_type = RequestType::RequestSync;
    peer::write(socket, request).await;
}

#[tokio::test]
async fn native_callbacks_export_bounded_state_duplicate_conflict_and_closed_journal() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut resource = fixture::resource();
    Arc::get_mut(&mut resource).unwrap().pause = true;
    let catalog = ResourceCatalog::new(4, 4096).unwrap();
    catalog.register(resource.clone()).unwrap();
    let store = fixture::journal().await;
    let rm = ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog,
        store.clone(),
    )
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let exporter = MetricsExporter::start(
        MetricsServerConfig::new("127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap(),
        MetricsSources::default().with_rm_branches(rm.metrics_sampler()),
    )
    .await
    .unwrap();
    let mut socket = registered(&listener).await;
    assert!(fetch(exporter.local_addr(), "wrong-rm-token")
        .await
        .starts_with("HTTP/1.1 401"));
    send_callback(&mut socket, -401, true).await;
    resource.entered.notified().await;
    let active = fetch(exporter.local_addr(), TOKEN).await;
    assert!(body(&active).contains("seata_rm_dispatch_active{phase=\"commit\"} 1"));
    assert!(
        body(&active).contains("seata_rm_journal_branches{phase=\"commit\",state=\"pending\"} 1")
    );
    resource.release.notify_one();
    let reply = peer::read(&mut socket).await;
    assert_eq!(reply.id, -401);
    assert!(
        matches!(reply.body,Some(Message::BranchCommitResponse(value))
        if value.response.result_code==ResultCode::Success && value.branch_status.to_java_v1()==5)
    );
    send_callback(&mut socket, -402, true).await;
    assert!(
        matches!(peer::read(&mut socket).await.body,Some(Message::BranchCommitResponse(value))
        if value.response.result_code==ResultCode::Success && value.branch_status.to_java_v1()==5)
    );
    send_callback(&mut socket, -403, false).await;
    assert!(
        matches!(peer::read(&mut socket).await.body,Some(Message::BranchRollbackResponse(value))
        if value.response.result_code==ResultCode::Failed && value.branch_status.to_java_v1()==10)
    );
    assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
    let retained = fetch(exporter.local_addr(), TOKEN).await;
    assert!(body(&retained).contains("seata_rm_journal_closed 0"));
    drop(store);
    let final_response = fetch(exporter.local_addr(), TOKEN).await;
    let text = body(&final_response);
    assert_eq!(text.lines().count(), 34);
    for expected in [
        "seata_rm_journal_closed 1",
        "seata_rm_journal_faulted 0",
        "seata_rm_journal_branches{phase=\"commit\",state=\"completed\"} 1",
        "seata_rm_dispatch_active{phase=\"commit\"} 0",
        "seata_rm_dispatch_total{phase=\"commit\",outcome=\"applied\"} 1",
        "seata_rm_dispatch_total{phase=\"commit\",outcome=\"duplicate_completed\"} 1",
        "seata_rm_dispatch_total{phase=\"rollback\",outcome=\"error\"} 1",
        "seata_rm_resource_attempts_total{phase=\"commit\",operation=\"apply\"} 1",
        "seata_rm_resource_attempts_total{phase=\"commit\",operation=\"recover\"} 0",
    ] {
        assert!(text.contains(expected), "{expected}");
    }
    for secret in [
        TOKEN,
        "wrong-rm-token",
        "tc:rm-private-identity",
        "123456789",
        "mysql://exact-resource",
        "private-application-credential",
        "D:/codex-builds",
    ] {
        assert!(!text.contains(secret));
    }
    exporter.shutdown().await.unwrap();
}
