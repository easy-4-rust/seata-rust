use crate::{JavaString, LowerCaseLinkHashMap, MetadataError, MetadataMapValue};
use std::sync::Arc;

/// Rust条目值别名：原规范化键与nullable共享值，不冒充Java的可变Entry对象。
/// 对应 Java: Map.Entry#getKey/getValue的读取结果。
pub type MetadataMapEntry<V> = (JavaString, Option<Arc<V>>);

/// Rust扩展：共享元数据Map的原插入顺序游标，next前检查结构版本，终点hasNext不额外检查。
/// 对应 Java: LinkedHashMap.EntryIterator的TableMeta使用契约；不是额外源对象。
pub struct MetadataEntryCursor<V: MetadataMapValue> {
    map: LowerCaseLinkHashMap<V>,
    version: i32,
    next: Option<usize>,
}

impl<V: MetadataMapValue> MetadataEntryCursor<V> {
    /// 用map、version及next建立内部游标，无回调或Map修改。
    /// 对应 Java: LinkedHashMap.EntryIterator初始化。
    pub(crate) fn new(map: LowerCaseLinkHashMap<V>, version: i32, next: Option<usize>) -> Self {
        Self { map, version, next }
    }
    /// 无参数，返回原hasNext结果，不因最后一个值回调发生结构修改而追加CME。
    /// 对应 Java: LinkedHashIterator#hasNext
    pub fn has_next(&self) -> bool {
        self.next.is_some()
    }
    /// 无参数，返回下一原始规范化key及nullable共享value；结束返回None，结构修改返回CME。
    /// 对应 Java: LinkedHashIterator#nextNode；返回的是Rust只读条目值而非完整Java可变Entry视图。
    pub fn next_entry(&mut self) -> Result<Option<MetadataMapEntry<V>>, MetadataError> {
        let Some(index) = self.next else {
            self.map.cursor_next(self.version, 0)?;
            return Ok(None);
        };
        let (next, key, value) = self.map.cursor_next(self.version, index)?;
        self.next = next;
        Ok(Some((key, value)))
    }
}
