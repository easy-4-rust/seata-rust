//! 真实 TCP 调用下校验 TM 结构化事件的关联字段与敏感字段边界。
#![allow(clippy::unwrap_used)]
#[path = "support/tm_peer.rs"]
mod fixture;

use fixture::{manager, peer, registered};
use seata_protocol::{Message, ResultCode};
use seata_tm::{TmError, Xid};
use std::collections::BTreeMap;
use std::fmt;
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

#[tokio::test(flavor = "current_thread")]
async fn tm_status_response_emits_correlated_event_without_request_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(Message::GlobalStatusRequest(_))
        ));
        peer::write(&mut socket, peer::reply(request.id, peer::status_result())).await;
        peer::eof(&mut socket).await;
    });
    let captured = Arc::new(Mutex::new(Vec::new()));
    let xid = Xid::parse("tc:structured-status".into()).unwrap();
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        tm.get_status(&xid).await.unwrap();
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_tm::rpc")
            })
            .unwrap();
        assert_eq!(fields.get("operation").map(String::as_str), Some("Status"));
        assert_eq!(fields.get("outcome").map(String::as_str), Some("response"));
        assert!(fields.get("xid").unwrap().contains(xid.as_str()));
        assert!(fields.get("request_id").unwrap().parse::<i32>().unwrap() > 0);
        assert_eq!(fields.len(), 6);
    }
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn tm_definite_rejection_is_logged_as_received_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        let Message::GlobalStatusResponse(mut response) = peer::status_result() else {
            panic!("status fixture changed");
        };
        response.response.result_code = ResultCode::Failed;
        peer::write(
            &mut socket,
            peer::reply(request.id, Message::GlobalStatusResponse(response)),
        )
        .await;
        peer::eof(&mut socket).await;
    });
    let captured = Arc::new(Mutex::new(Vec::new()));
    let xid = Xid::parse("tc:rejected-status".into()).unwrap();
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        assert!(matches!(
            tm.get_status(&xid).await,
            Err(TmError::DefiniteRejected { .. })
        ));
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_tm::rpc")
            })
            .unwrap();
        assert_eq!(fields.get("outcome").map(String::as_str), Some("response"));
    }
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn tm_lost_status_response_emits_unknown_outcome_without_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tm = manager(listener.local_addr().unwrap(), Duration::from_secs(2));
    let server = tokio::spawn(async move {
        let mut socket = registered(&listener).await;
        let request = peer::read(&mut socket).await;
        assert!(matches!(
            request.body,
            Some(Message::GlobalStatusRequest(_))
        ));
        drop(socket);
    });
    let captured = Arc::new(Mutex::new(Vec::new()));
    let xid = Xid::parse("tc:unknown-status".into()).unwrap();
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        assert!(tm.get_status(&xid).await.unwrap_err().is_unknown_outcome());
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_tm::rpc")
            })
            .unwrap();
        assert_eq!(fields.get("outcome").map(String::as_str), Some("unknown"));
        assert!(fields.get("xid").unwrap().contains(xid.as_str()));
        assert!(fields.get("request_id").unwrap().starts_with("Some("));
        assert_eq!(fields.len(), 6);
    }
    tm.shutdown().await.unwrap();
    server.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn tm_unreachable_peer_emits_not_sent_without_invented_request_id() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let tm = manager(address, Duration::from_millis(80));
    let captured = Arc::new(Mutex::new(Vec::new()));
    let xid = Xid::parse("tc:not-sent-status".into()).unwrap();
    {
        let _guard = tracing::subscriber::set_default(EventCapture(Arc::clone(&captured)));
        assert!(matches!(
            tm.get_status(&xid).await,
            Err(TmError::LocalFailureBeforeSend { .. })
        ));
    }
    {
        let events = captured.lock().unwrap();
        let fields = events
            .iter()
            .find(|fields| {
                fields
                    .get("target")
                    .is_some_and(|value| value == "seata_tm::rpc")
            })
            .unwrap();
        assert_eq!(fields.get("outcome").map(String::as_str), Some("not_sent"));
        assert_eq!(fields.get("request_id").map(String::as_str), Some("None"));
        assert!(fields.get("xid").unwrap().contains(xid.as_str()));
    }
    tm.shutdown().await.unwrap();
}
