use crate::{JavaString, LowerCaseLinkHashMap, MetadataError, MetadataMapValue};
use std::sync::Arc;

/// 接收一次访问中的可空键和值，失败时终止当前遍历。
/// 对应 Java: Map.forEach 的 BiConsumer。
pub type MetadataMapVisitor<'a, V> =
    dyn FnMut(Option<&JavaString>, Option<Arc<V>>) -> Result<(), MetadataError> + 'a;

/// 能按自身顺序访问条目的批量写入来源，可在不同遍历之间改变。
/// 对应 Java: Map#forEach 作为 LowerCaseLinkHashMap#putAll 的来源。
pub trait MetadataMapSource<V: MetadataMapValue> {
    /// 调用 visitor 访问当前条目，返回来源或访问器产生的错误。
    /// 对应 Java: Map#forEach。
    fn for_each_entry(&self, visitor: &mut MetadataMapVisitor<'_, V>) -> Result<(), MetadataError>;
}

impl<V: MetadataMapValue> MetadataMapSource<V> for LowerCaseLinkHashMap<V> {
    /// 按目标 Map 的插入顺序访问规范化键与可空共享值。
    /// 对应 Java: LowerCaseLinkHashMap 继承的 Map#forEach。
    fn for_each_entry(&self, visitor: &mut MetadataMapVisitor<'_, V>) -> Result<(), MetadataError> {
        self.for_each(Some(|key: &JavaString, value| visitor(Some(key), value)))
    }
}
