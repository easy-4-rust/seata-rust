use crate::StructuredLogSubscriber;
use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, Layer};

/// 组合模式下只观察正式 Seata 事件的桥接层，不生成 span 身份或处理应用 span 字段。
/// 对应 Java: 无直接对象；WP07 Rust 应用日志栈组合扩展。
#[derive(Debug, Clone)]
pub(crate) struct StructuredLogLayer {
    subscriber: StructuredLogSubscriber,
}
impl StructuredLogLayer {
    /// 参数为原订阅器；返回复用同一队列、预算及计数的事件桥接层。
    pub(crate) fn new(subscriber: StructuredLogSubscriber) -> Self {
        Self { subscriber }
    }
}
impl<S: Subscriber> Layer<S> for StructuredLogLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        if self.subscriber.enabled(event.metadata()) {
            self.subscriber.event(event);
        }
    }
}
