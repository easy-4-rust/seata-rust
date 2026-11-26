use crate::{LowerCaseLinkHashMap, MetadataError, MetadataMapValue, MetadataValuesCursor};

/// 目标 Map 的实时值集合。
/// 对应 Java: LowerCaseLinkHashMap#values。
pub struct MetadataValues<V: MetadataMapValue>(pub(crate) LowerCaseLinkHashMap<V>);

impl<V: MetadataMapValue> MetadataValues<V> {
    /// 返回目标 Map 的实时值数量，包含 null。
    /// 对应 Java: LinkedHashMap.LinkedValues#size。
    pub fn len(&self) -> Result<usize, MetadataError> {
        self.0.len()
    }

    /// 返回目标 Map 当前是否为空。
    /// 对应 Java: LinkedHashMap.LinkedValues#isEmpty。
    pub fn is_empty(&self) -> Result<bool, MetadataError> {
        self.0.is_empty()
    }

    /// 建立按原插入顺序读取值的迭代器。
    /// 对应 Java: LinkedHashMap.LinkedValues#iterator。
    pub fn iter(&self) -> Result<MetadataValuesCursor<V>, MetadataError> {
        self.0.entries().map(MetadataValuesCursor)
    }

    /// 仅清空目标 Map，保留原始拼写。
    /// 对应 Java: LinkedHashMap.LinkedValues#clear。
    pub fn clear(&self) -> Result<(), MetadataError> {
        self.0.clear_target()
    }

    /// 按可空 value 的比较方向移除第一个匹配值，保留原始键映射。
    /// 对应 Java: AbstractCollection#remove。
    pub fn remove(&self, value: Option<&V>) -> Result<bool, MetadataError> {
        let mut cursor = self.iter()?;
        while cursor.has_next() {
            let current = cursor.next_value()?;
            let matched = match value {
                Some(value) => value.metadata_equals(current.as_deref())?,
                None => current.is_none(),
            };
            if matched {
                cursor.remove_current()?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}
