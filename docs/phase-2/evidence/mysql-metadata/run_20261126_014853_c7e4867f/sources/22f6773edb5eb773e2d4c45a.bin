use crate::{
    JavaString, LowerCaseLinkHashMap, MetadataEntryCursor, MetadataError, MetadataMapValue,
};

/// 目标 Map 的实时条目视图，键使用规范化拼写。
/// 对应 Java: LowerCaseLinkHashMap#entrySet。
pub struct MetadataEntrySet<V: MetadataMapValue>(pub(crate) LowerCaseLinkHashMap<V>);

impl<V: MetadataMapValue> MetadataEntrySet<V> {
    /// 返回当前目标条目数。
    /// 对应 Java: LinkedHashMap.LinkedEntrySet#size。
    pub fn len(&self) -> Result<usize, MetadataError> {
        self.0.len()
    }

    /// 判断当前目标 Map 是否为空。
    /// 对应 Java: LinkedHashMap.LinkedEntrySet#isEmpty。
    pub fn is_empty(&self) -> Result<bool, MetadataError> {
        self.0.is_empty()
    }

    /// 建立实时条目迭代器。
    /// 对应 Java: LinkedHashMap.LinkedEntrySet#iterator。
    pub fn iter(&self) -> Result<MetadataEntryCursor<V>, MetadataError> {
        self.0.entries()
    }

    /// 仅清空目标 Map，保留原始拼写。
    /// 对应 Java: LinkedHashMap.LinkedEntrySet#clear。
    pub fn clear(&self) -> Result<(), MetadataError> {
        self.0.clear_target()
    }

    /// 用未转换的 key 和可空 value 匹配并移除目标节点。
    /// 对应 Java: LinkedHashMap.LinkedEntrySet#remove。
    pub fn remove(&self, key: &JavaString, value: Option<&V>) -> Result<bool, MetadataError> {
        let Some((index, entry)) = self.0.find_entry(key)? else {
            return Ok(false);
        };
        let current = entry.value()?;
        if !crate::metadata_map_value::value_matches(value, current.as_deref())? {
            return Ok(false);
        }
        self.0.remove_entry_if_same(index, &entry)
    }
}
