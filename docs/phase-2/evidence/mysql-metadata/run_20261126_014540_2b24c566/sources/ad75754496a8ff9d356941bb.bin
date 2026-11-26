//! 真实 TCP 下检查 RM 出站事件的关联字段与资源摘要边界。
#![allow(clippy::unwrap_used)]
#[path = "support/rm_peer.rs"]
mod fixture;

use fixture::{manager, peer, registered};
use seata_protocol::protocol::{MergedWarpMessage, RegisterRMResponse};
use seata_protocol::transaction::{
    BranchCommitRequest, BranchReportRequest, BranchReportResponse, BranchRollbackRequest,
    GlobalStatusRequest,
};
use seata_protocol::{BranchStatus, BranchType, Message, RequestType, ResultCode};
use seata_rm::{ResourceCatalog, ResourceManager, RmCallError};
use seata_transport::ResourceKey;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

#[derive(Clone)]
struct EventCapture(Arc<Mutex<Vec<BTreeMap<String, String>>>>);

struct EventFields(BTreeMap<String, String>);

impl Visit for EventFields {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.insert(field.name().into(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name().into(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().into(), value.to_string());
    }
}

impl Subscriber for EventCapture {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = EventFields(BTreeMap::new());
        event.record(&mut fields);
        fields
            .0
            .insert("target".into(), event.metadata().target().into());
        self.0.lock().unwrap().push(fields.0);
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

fn report_request() -> BranchReportRequest {
    BranchReportRequest {
        xid: Some("tc:branch-event".into()),
        branch_id: 77,
        status: BranchStatus::PhaseOneDone,
        resource_id: Some("mysql://exact-resource".into()),
        application_data: Some("credential-probe".into()),
        branch_type: BranchType::At,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn branch_report_event_uses_resource_digest_and_omits_application_data() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        let Some(Message::BranchReportRequest(value)) = request.body else {
            panic!("expected BranchReportRequest");
        };
        assert_eq!(value.branch_id, 77);
        assert_eq!(value.application_data.as_deref(), Some("credential-probe"));
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: peer::success(),
                }),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        rm.client().branch_report(report_request()).await.unwrap();
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_rm::rpc")
            })
            .unwrap();
        assert_eq!(
            fields.get("operation").map(String::as_str),
            Some("BranchReport")
        );
        assert_eq!(fields.get("outcome").map(String::as_str), Some("response"));
        assert!(fields.get("xid").unwrap().contains("tc:branch-event"));
        assert!(fields.get("branch_id").unwrap().contains("77"));
        assert!(fields.get("request_id").unwrap().parse::<i32>().unwrap() > 0);
        assert!(fields
            .get("resource_sha256")
            .unwrap()
            .contains("b979c9643e641c11d295b0df0d3f7ac98505f0e881e22676601048700d3e1792"));
        assert_eq!(fields.len(), 8);
        let text = format!("{fields:?}");
        assert!(!text.contains("mysql://exact-resource"));
        assert!(!text.contains("credential-probe"));
    }
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn branch_report_rejection_stays_a_received_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        let mut prefix = peer::success();
        prefix.result_code = ResultCode::Failed;
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                Message::BranchReportResponse(BranchReportResponse { response: prefix }),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        assert!(matches!(
            rm.client().branch_report(report_request()).await,
            Err(RmCallError::DefiniteRejected { .. })
        ));
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_rm::rpc")
            })
            .unwrap();
        assert_eq!(fields.get("outcome").map(String::as_str), Some("response"));
    }
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn branch_report_lost_response_emits_unknown_without_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, _) = manager(&listener, Duration::from_secs(2)).await;
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(Message::BranchReportRequest(_))
        ));
        drop(socket);
    });
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        assert!(rm
            .client()
            .branch_report(report_request())
            .await
            .unwrap_err()
            .is_unknown_outcome());
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_rm::rpc")
            })
            .unwrap();
        assert_eq!(fields.get("outcome").map(String::as_str), Some("unknown"));
        assert!(fields.get("request_id").unwrap().starts_with("Some("));
        assert_eq!(fields.len(), 8);
        assert!(!format!("{fields:?}").contains("credential-probe"));
    }
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn branch_report_unreachable_peer_emits_not_sent_without_request_id() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let catalog = ResourceCatalog::new(4, 4096).unwrap();
    catalog.register(fixture::resource()).unwrap();
    let rm = ResourceManager::builder(
        fixture::settings(address),
        fixture::registration(),
        catalog,
        fixture::journal().await,
    )
    .request_timeout(Duration::from_millis(80))
    .build()
    .unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        assert!(matches!(
            rm.client().branch_report(report_request()).await,
            Err(RmCallError::LocalFailureBeforeSend { .. })
        ));
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_rm::rpc")
            })
            .unwrap();
        assert_eq!(fields.get("outcome").map(String::as_str), Some("not_sent"));
        assert_eq!(fields.get("request_id").map(String::as_str), Some("None"));
        assert!(fields.get("branch_id").unwrap().contains("77"));
        assert!(!format!("{fields:?}").contains("credential-probe"));
    }
    rm.shutdown().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn credential_bearing_resource_is_hashed_in_rm_event() {
    const RESOURCE: &str =
        "mysql://trace_user:credential_needle@127.0.0.1/orders?token=credential_needle";
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let catalog = ResourceCatalog::new(4, 4096).unwrap();
    catalog
        .register(Arc::new(fixture::Resource {
            key: ResourceKey::new(BranchType::At, RESOURCE.into()).unwrap(),
            actions: AtomicU64::new(0),
            stall: false,
            entered: tokio::sync::Notify::new(),
            pause: false,
            release: tokio::sync::Notify::new(),
            panic_on_drop: false,
        }))
        .unwrap();
    let rm = ResourceManager::builder(
        fixture::settings(listener.local_addr().unwrap()),
        fixture::registration(),
        catalog,
        fixture::journal().await,
    )
    .request_timeout(Duration::from_secs(2))
    .build()
    .unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = peer::read(&mut socket).await;
        let Some(Message::RegisterRMRequest(value)) = request.body else {
            panic!("expected RegisterRMRequest");
        };
        assert_eq!(value.resource_ids.as_deref(), Some(RESOURCE));
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                Message::RegisterRMResponse(RegisterRMResponse {
                    identified: true,
                    wire_version: None,
                }),
            ),
        )
        .await;
        let request = peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(Message::BranchReportRequest(_))
        ));
        peer::write(
            &mut socket,
            peer::reply(
                request.id,
                Message::BranchReportResponse(BranchReportResponse {
                    response: peer::success(),
                }),
            ),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        let mut request = report_request();
        request.resource_id = Some(RESOURCE.into());
        rm.client().branch_report(request).await.unwrap();
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_rm::rpc")
            })
            .unwrap();
        assert!(fields
            .get("resource_sha256")
            .unwrap()
            .contains("f560edf54875e58e5415294194cf9efb42864143a42662016148be11caa9d1d3"));
        let text = format!("{fields:?}");
        assert!(!text.contains(RESOURCE));
        assert!(!text.contains("credential_needle"));
        assert!(!text.contains("trace_user"));
    }
    rm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn callback_decisions_emit_original_branch_identity_without_replaying_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        for (request_id, commit, expected_status) in [
            (-31, true, BranchStatus::PhaseTwoCommitted),
            (-32, true, BranchStatus::PhaseTwoCommitted),
            (-33, false, BranchStatus::PhaseTwoRollbackFailedUnretryable),
        ] {
            let body = if commit {
                Message::BranchCommitRequest(BranchCommitRequest {
                    xid: "tc:callback-event".into(),
                    branch_id: 91,
                    branch_type: BranchType::At,
                    resource_id: "mysql://exact-resource".into(),
                    application_data: Some("callback-secret".into()),
                })
            } else {
                Message::BranchRollbackRequest(BranchRollbackRequest {
                    xid: "tc:callback-event".into(),
                    branch_id: 91,
                    branch_type: BranchType::At,
                    resource_id: "mysql://exact-resource".into(),
                    application_data: Some("callback-secret".into()),
                })
            };
            let mut request = peer::reply(request_id, body);
            request.message_type = RequestType::RequestSync;
            peer::write(&mut socket, request).await;
            let response = peer::read(&mut socket).await;
            assert_eq!(response.id, request_id);
            let observed_status = match response.body.unwrap() {
                Message::BranchCommitResponse(value) => value.branch_status,
                Message::BranchRollbackResponse(value) => value.branch_status,
                other => panic!("unexpected callback response: {other:?}"),
            };
            assert_eq!(observed_status, expected_status);
        }
    }
    assert_eq!(resource.actions.load(Ordering::SeqCst), 1);
    {
        let events = captured.lock().unwrap();
        let callbacks: Vec<_> = events
            .iter()
            .filter(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_rm::callback")
            })
            .collect();
        assert_eq!(callbacks.len(), 3);
        for (index, fields) in callbacks.into_iter().enumerate() {
            assert!(fields.get("xid").unwrap().contains("tc:callback-event"));
            assert!(fields.get("branch_id").unwrap().contains("91"));
            assert_eq!(
                fields.get("request_id").map(String::as_str),
                Some(["-31", "-32", "-33"][index])
            );
            assert_eq!(
                fields.get("phase").map(String::as_str),
                Some(["Commit", "Commit", "Rollback"][index])
            );
            assert_eq!(
                fields.get("branch_status").map(String::as_str),
                Some(
                    [
                        "PhaseTwoCommitted",
                        "PhaseTwoCommitted",
                        "PhaseTwoRollbackFailedUnretryable",
                    ][index]
                )
            );
            assert_eq!(
                fields.get("result_code").map(String::as_str),
                Some(["Success", "Success", "Failed"][index])
            );
            assert!(fields
                .get("resource_sha256")
                .unwrap()
                .contains("b979c9643e641c11d295b0df0d3f7ac98505f0e881e22676601048700d3e1792"));
            assert_eq!(fields.len(), 9);
            assert!(!format!("{fields:?}").contains("callback-secret"));
            assert!(!format!("{fields:?}").contains("mysql://exact-resource"));
        }
    }
    rm.shutdown().await.unwrap();
    peer::eof(&mut socket).await;
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_callback_batch_emits_no_branch_decision() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (rm, resource) = manager(&listener, Duration::from_secs(2)).await;
    let mut socket = registered(&listener).await;
    let captured = Arc::new(Mutex::new(Vec::new()));
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        let mut request = peer::reply(
            -44,
            Message::MergedWarpMessage(MergedWarpMessage {
                msgs: vec![
                    Message::BranchCommitRequest(BranchCommitRequest {
                        xid: "tc:callback-event".into(),
                        branch_id: 91,
                        branch_type: BranchType::At,
                        resource_id: "mysql://exact-resource".into(),
                        application_data: Some("callback-secret".into()),
                    }),
                    Message::GlobalStatusRequest(GlobalStatusRequest {
                        xid: Some("tc:callback-event".into()),
                        extra_data: None,
                    }),
                ],
                msg_ids: vec![],
            }),
        );
        request.message_type = RequestType::RequestSync;
        peer::write(&mut socket, request).await;
        peer::eof(&mut socket).await;
    }
    assert_eq!(resource.actions.load(Ordering::SeqCst), 0);
    assert!(!captured.lock().unwrap().iter().any(|fields| {
        fields
            .get("target")
            .is_some_and(|value| value == "seata_rm::callback")
    }));
    rm.shutdown().await.unwrap();
}
