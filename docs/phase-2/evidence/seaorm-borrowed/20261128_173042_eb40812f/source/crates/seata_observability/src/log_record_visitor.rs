use crate::log_record_buffer::LogRecordBuffer;
use std::{collections::BTreeMap, fmt};
use tracing::field::{Field, Visit};
/// 只在静态元数据模板已验证后捕获字段；共享总字段字节预算，禁止部分记录输出。
/// 对应 Java: 无直接对象；WP07 Rust 结构化日志字段访问器。
pub(crate) struct LogRecordVisitor {
    pub(crate) fields: BTreeMap<String, String>,
    pub(crate) oversize: bool,
    remaining: usize,
}
impl LogRecordVisitor {
    /// 参数为总字段内容预算；返回空字段集合。
    pub(crate) fn new(maximum: usize) -> Self {
        Self {
            fields: BTreeMap::new(),
            oversize: false,
            remaining: maximum,
        }
    }
    fn insert(&mut self, field: &Field, value: &str) {
        if self.oversize {
            return;
        }
        if value.len() > self.remaining {
            self.oversize = true;
            return;
        }
        self.remaining -= value.len();
        self.fields.insert(field.name().into(), value.into());
    }
}
impl Visit for LogRecordVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field, value);
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.oversize {
            return;
        }
        let mut buffer = LogRecordBuffer::new(self.remaining);
        if fmt::write(&mut buffer, format_args!("{value:?}")).is_err() {
            self.oversize = true;
            return;
        }
        match String::from_utf8(buffer.into_bytes()) {
            Ok(value) => self.insert(field, &value),
            Err(_) => self.oversize = true,
        }
    }
}
