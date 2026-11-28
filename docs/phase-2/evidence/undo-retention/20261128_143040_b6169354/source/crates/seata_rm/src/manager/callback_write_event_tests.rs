#![allow(clippy::unwrap_used)]
use super::CallbackWriteEvent;
use seata_transport::{CallError, ConnectionConfig, TcpConnection, TransportError};
use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::{
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    Event, Metadata, Subscriber,
};

struct Capture(Arc<Mutex<Vec<BTreeMap<String, String>>>>);
struct Fields(BTreeMap<String, String>);
impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.insert(field.name().into(), format!("{value:?}"));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().into(), value.into());
    }
}
impl Subscriber for Capture {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        if event.metadata().target() == "seata_rm::callback_write" {
            let mut fields = Fields(BTreeMap::new());
            event.record(&mut fields);
            self.0.lock().unwrap().push(fields.0);
        }
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

#[tokio::test(flavor = "current_thread")]
async fn original_result_and_cancelled_scope_each_emit_exactly_one_bounded_event() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let limits = seata_protocol::v1::FrameLimits::new(
        65536,
        32767,
        128,
        seata_protocol::PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
    )
    .unwrap();
    let config =
        ConnectionConfig::new(limits, 4, Duration::from_secs(2), Duration::from_secs(2)).unwrap();
    let connection = TcpConnection::connect(listener.local_addr().unwrap(), config)
        .await
        .unwrap();
    let generation = connection.handle().generation();
    let values = Arc::new(Mutex::new(Vec::new()));
    {
        let _subscriber = tracing::subscriber::set_default(Capture(values.clone()));
        for (id, result) in [
            (-1, Ok(())),
            (
                -2,
                Err(CallError::LocalFailureBeforeSend {
                    generation: Some(generation),
                    request_id: Some(-2),
                    reason: TransportError::ConnectionClosed,
                }),
            ),
            (
                -3,
                Err(CallError::UnknownOutcome {
                    generation,
                    request_id: -3,
                    reason: TransportError::WriteTimeout,
                }),
            ),
        ] {
            let original = result.clone();
            CallbackWriteEvent::new(id, generation.value(), 2).finish(&result);
            assert_eq!(result, original);
        }
        let entered = tokio::sync::Notify::new();
        let pending = async {
            let _event = CallbackWriteEvent::new(-4, generation.value(), 2);
            entered.notify_one();
            std::future::pending::<()>().await;
        };
        let mut pending = Box::pin(pending);
        tokio::select! { biased; _ = &mut pending => panic!("must stay pending"), _ = entered.notified() => {} }
        drop(pending);
        assert!(std::panic::catch_unwind(|| {
            let _event = CallbackWriteEvent::new(-5, generation.value(), 2);
            panic!("fixture unwind");
        })
        .is_err());
    }
    {
        let events = values.lock().unwrap();
        assert_eq!(events.len(), 5);
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event.len(), 5);
            assert_eq!(event["request_id"], (-(index as i32 + 1)).to_string());
            assert_eq!(
                event["connection_generation"],
                generation.value().to_string()
            );
            assert_eq!(event["response_count"], "2");
            assert_eq!(
                event["outcome"],
                [
                    "local_written",
                    "not_sent",
                    "unknown",
                    "cancelled",
                    "cancelled"
                ][index]
            );
        }
    }
    connection.shutdown().await.unwrap();
}
