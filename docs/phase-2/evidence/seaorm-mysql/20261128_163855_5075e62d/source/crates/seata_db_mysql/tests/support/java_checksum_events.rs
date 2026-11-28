//! 捕获现场 MySQL 摘要校验事件，核对关联字段和敏感信息边界。
use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};
use tracing::{
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    Event, Metadata, Subscriber,
};
#[derive(Clone)]
pub(super) struct EventCapture(pub(super) Arc<Mutex<Vec<BTreeMap<String, String>>>>);

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
