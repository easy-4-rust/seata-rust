use crate::{JavaString, MetadataError, MetadataMapValue};
use std::sync::{Arc, RwLock};

/// 目标 Map 条目的共享句柄，脱离 Map 后仍保留键和值。
/// 对应 Java: LinkedHashMap.Entry 的 getKey、getValue 和 setValue。
pub struct MetadataLiveEntry<V: MetadataMapValue> {
    key: JavaString,
    value: Arc<RwLock<Option<Arc<V>>>>,
}

impl<V: MetadataMapValue> Clone for MetadataLiveEntry<V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

impl<V: MetadataMapValue> MetadataLiveEntry<V> {
    /// 从规范化 key 和可空 value 建立独立条目节点。
    /// 对应 Java: LinkedHashMap.Entry 构造器。
    pub(crate) fn new(key: JavaString, value: Option<Arc<V>>) -> Self {
        Self {
            key,
            value: Arc::new(RwLock::new(value)),
        }
    }
    /// 返回条目的原始规范化键。
    /// 对应 Java: Map.Entry#getKey。
    pub fn key(&self) -> &JavaString {
        &self.key
    }

    /// 返回当前共享值，包含已脱离 Map 的条目。
    /// 对应 Java: Map.Entry#getValue。
    pub fn value(&self) -> Result<Option<Arc<V>>, MetadataError> {
        Ok(self
            .value
            .read()
            .map_err(|_| MetadataError::LockPoisoned("MetadataLiveEntry"))?
            .clone())
    }

    /// 设置条目的可空 value，返回原值。此操作不修改 Map 结构版本。
    /// 对应 Java: Map.Entry#setValue。
    pub fn set_value(&self, value: Option<Arc<V>>) -> Result<Option<Arc<V>>, MetadataError> {
        let mut current = self
            .value
            .write()
            .map_err(|_| MetadataError::LockPoisoned("MetadataLiveEntry"))?;
        Ok(std::mem::replace(&mut *current, value))
    }

    /// 判断 other 是否为同一个目标节点。
    /// 对应 Java: LinkedHashMap.Entry 引用身份。
    pub(crate) fn same_node(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}
