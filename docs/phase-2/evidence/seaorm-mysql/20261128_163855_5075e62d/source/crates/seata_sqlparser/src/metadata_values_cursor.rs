use crate::{MetadataEntryCursor, MetadataError, MetadataMapValue};
use std::sync::Arc;

/// 实时值迭代器，复用 LinkedHashMap 的条目顺序和结构版本。
/// 对应 Java: LinkedHashMap.LinkedValueIterator。
pub struct MetadataValuesCursor<V: MetadataMapValue>(pub(crate) MetadataEntryCursor<V>);

impl<V: MetadataMapValue> MetadataValuesCursor<V> {
    /// 检查是否还有后续值，不做结构版本校验。
    /// 对应 Java: LinkedHashIterator#hasNext。
    pub fn has_next(&self) -> bool {
        self.0.has_next()
    }

    /// 读取下一个可空值，结束时返回原异常。
    /// 对应 Java: LinkedValueIterator#next。
    pub fn next_value(&mut self) -> Result<Option<Arc<V>>, MetadataError> {
        self.0.next_handle()?.value()
    }

    /// 移除上次读取的目标节点。
    /// 对应 Java: LinkedHashIterator#remove。
    pub fn remove_current(&mut self) -> Result<(), MetadataError> {
        self.0.remove_current()
    }
}
