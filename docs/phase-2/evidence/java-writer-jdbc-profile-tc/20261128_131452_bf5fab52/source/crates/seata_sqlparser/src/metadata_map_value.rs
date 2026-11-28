use crate::{ColumnMeta, IndexMeta, IndexType, MetadataError};

/// 用查询值比较目标值，优先处理引用相同及 null。
/// 对应 Java: HashMap#removeNode 和 AbstractCollection#remove。
pub(crate) fn value_matches<V: MetadataMapValue>(
    value: Option<&V>,
    target: Option<&V>,
) -> Result<bool, MetadataError> {
    match (value, target) {
        (None, target) => Ok(target.is_none()),
        (Some(value), Some(target)) if std::ptr::eq(value, target) => Ok(true),
        (Some(value), target) => value.metadata_equals(target),
    }
}

/// Rust扩展：元数据Map在锁外调用值的Java相等/哈希语义，不把失败吞成false或0。
/// 对应 Java: Map内ColumnMeta/IndexMeta的Object方法调用边界；不计原31对象。
pub trait MetadataMapValue: Send + Sync + 'static {
    /// 比较可空other并返回结果；原动态回调失败原样返回。
    /// 对应 Java: Map.equals中value.equals(otherValue)
    fn metadata_equals(&self, other: Option<&Self>) -> Result<bool, MetadataError>;
    /// 用enum_identity上下文计算Java式hash；回调均在Map锁外执行。
    /// 对应 Java: Map.Entry#hashCode中的值hash及Enum身份上下文。
    fn metadata_hash(
        &self,
        enum_identity: &mut dyn FnMut(IndexType) -> i32,
    ) -> Result<i32, MetadataError>;
}

impl MetadataMapValue for ColumnMeta {
    fn metadata_equals(&self, other: Option<&Self>) -> Result<bool, MetadataError> {
        self.java_equals(other.map(|v| v as &dyn std::any::Any))
    }
    fn metadata_hash(
        &self,
        _enum_identity: &mut dyn FnMut(IndexType) -> i32,
    ) -> Result<i32, MetadataError> {
        self.java_hash_code()
    }
}
impl MetadataMapValue for IndexMeta {
    fn metadata_equals(&self, other: Option<&Self>) -> Result<bool, MetadataError> {
        self.java_equals(other.map(|v| v as &dyn std::any::Any))
    }
    fn metadata_hash(
        &self,
        enum_identity: &mut dyn FnMut(IndexType) -> i32,
    ) -> Result<i32, MetadataError> {
        self.java_hash_code_with_enum_identity(enum_identity)
    }
}
