use crate::{JavaString, LowerCaseLinkHashMap, MetadataError, MetadataLiveEntry, MetadataMapValue};
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
    current: Option<usize>,
}

impl<V: MetadataMapValue> MetadataEntryCursor<V> {
    /// 用map、version及next建立内部游标，无回调或Map修改。
    /// 对应 Java: LinkedHashMap.EntryIterator初始化。
    pub(crate) fn new(map: LowerCaseLinkHashMap<V>, version: i32, next: Option<usize>) -> Self {
        Self {
            map,
            version,
            next,
            current: None,
        }
    }
    /// 无参数，返回原hasNext结果，不因最后一个值回调发生结构修改而追加CME。
    /// 对应 Java: LinkedHashIterator#hasNext
    pub fn has_next(&self) -> bool {
        self.next.is_some()
    }
    /// 无参数，返回下一原始规范化key及nullable共享value；结束返回None，结构修改返回CME。
    /// 对应 Java: LinkedHashIterator#nextNode；返回的是Rust只读条目值而非完整Java可变Entry视图。
    pub fn next_entry(&mut self) -> Result<Option<MetadataMapEntry<V>>, MetadataError> {
        if self.next.is_none() {
            return Ok(None);
        }
        let entry = self.next_handle()?;
        Ok(Some((entry.key().clone(), entry.value()?)))
    }
    /// 返回下一个实时条目，结构修改优先于迭代耗尽报告。
    /// 对应 Java: LinkedHashMap.LinkedEntryIterator#next。
    pub fn next_handle(&mut self) -> Result<MetadataLiveEntry<V>, MetadataError> {
        let Some(index) = self.next else {
            self.map.cursor_check(self.version)?;
            return Err(MetadataError::IteratorExhausted);
        };
        let (next, entry) = self.map.cursor_next(self.version, index)?;
        self.next = next;
        self.current = Some(index);
        Ok(entry)
    }
    /// 移除最近返回的目标节点并更新游标版本。
    /// 对应 Java: LinkedHashIterator#remove。
    pub fn remove_current(&mut self) -> Result<(), MetadataError> {
        let index = self.current.ok_or(MetadataError::IteratorState)?;
        self.version = self.map.cursor_remove(self.version, index)?;
        self.current = None;
        Ok(())
    }
}
