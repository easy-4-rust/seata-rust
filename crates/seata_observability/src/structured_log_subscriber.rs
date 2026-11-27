use crate::structured_log_sampler::{ACCEPTED, CLOSED, FULL, INVALID, OVERSIZE};
use crate::{
    log_record_buffer::LogRecordBuffer, log_record_visitor::LogRecordVisitor, structured_log_schema,
};
use crate::{StructuredLogConfig, StructuredLogReceiver, StructuredLogSampler};
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;
use tracing::{
    span::{Attributes, Id, Record},
    subscriber::NoSubscriber,
    Event, Metadata, Subscriber,
};

/// 显式安装的固定事件 JSON 订阅器；仅执行有界内存编码及非阻塞入队，不执行外部 I/O。
/// 对应 Java: 无直接对象；WP07 Rust 结构化日志订阅与交付扩展。
#[derive(Clone)]
pub struct StructuredLogSubscriber {
    sender: mpsc::Sender<Vec<u8>>,
    maximum_record_bytes: usize,
    sampler: StructuredLogSampler,
}
impl StructuredLogSubscriber {
    /// 参数为已验证预算；返回订阅器、唯一消费者及只读采样器，不自动安装全局 Dispatch。
    pub fn channel(
        config: StructuredLogConfig,
    ) -> (Self, StructuredLogReceiver, StructuredLogSampler) {
        let (sender, receiver) = mpsc::channel(config.capacity);
        let sampler = StructuredLogSampler::default();
        (
            Self {
                sender,
                maximum_record_bytes: config.maximum_record_bytes,
                sampler: sampler.clone(),
            },
            StructuredLogReceiver {
                receiver,
                sampler: sampler.clone(),
            },
            sampler,
        )
    }
}
impl fmt::Debug for StructuredLogSubscriber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StructuredLogSubscriber")
            .field("maximum_record_bytes", &self.maximum_record_bytes)
            .finish_non_exhaustive()
    }
}
impl Subscriber for StructuredLogSubscriber {
    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        meta.is_event() && structured_log_schema::known_target(meta.target())
    }
    // 本适配器不收集任意 span 字段；将所需 trait 操作明确委托给标准禁用实现。
    fn new_span(&self, attrs: &Attributes<'_>) -> Id {
        NoSubscriber::default().new_span(attrs)
    }
    fn record(&self, id: &Id, record: &Record<'_>) {
        NoSubscriber::default().record(id, record);
    }
    fn record_follows_from(&self, id: &Id, follows: &Id) {
        NoSubscriber::default().record_follows_from(id, follows);
    }
    fn enter(&self, id: &Id) {
        NoSubscriber::default().enter(id);
    }
    fn exit(&self, id: &Id) {
        NoSubscriber::default().exit(id);
    }
    fn event(&self, event: &Event<'_>) {
        let meta = event.metadata();
        let Some(message) = structured_log_schema::expected_message(meta) else {
            self.sampler.record(INVALID);
            return;
        };
        let mut visitor = LogRecordVisitor::new(self.maximum_record_bytes);
        event.record(&mut visitor);
        if visitor.oversize {
            self.sampler.record(OVERSIZE);
            return;
        }
        if visitor.fields.len() != meta.fields().len()
            || !structured_log_schema::valid_values(meta.target(), &visitor.fields, message)
        {
            self.sampler.record(INVALID);
            return;
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| u64::try_from(d.as_millis()).ok())
            .unwrap_or(0);
        let value = serde_json::json!({"target":meta.target(),"level":meta.level().as_str(),"timestamp_unix_millis":timestamp,"fields":visitor.fields});
        let mut buffer = LogRecordBuffer::new(self.maximum_record_bytes - 1);
        if serde_json::to_writer(&mut buffer, &value).is_err() {
            self.sampler.record(OVERSIZE);
            return;
        }
        let mut bytes = buffer.into_bytes();
        bytes.push(b'\n');
        match self.sender.try_send(bytes) {
            Ok(()) => self.sampler.record(ACCEPTED),
            Err(mpsc::error::TrySendError::Full(_)) => self.sampler.record(FULL),
            Err(mpsc::error::TrySendError::Closed(_)) => self.sampler.record(CLOSED),
        }
    }
}
