use crate::TransportError;
use seata_protocol::BranchType;
use std::sync::Arc;
/// 资源模式和精确原始标识构成的缓存键，不规范化JDBC URL或混用不同分支模式。
/// 对应 Java: AbstractResource#getBranchType/getResourceId 的组合身份。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ResourceKey {
    branch_type: BranchType,
    resource_id: Arc<str>,
}
impl ResourceKey {
    /// 参数是分支模式和原始资源ID；返回不为空且不会被Java逗号分隔器拆分的键。
    pub fn new(branch_type: BranchType, resource_id: String) -> Result<Self, TransportError> {
        if resource_id.is_empty() || resource_id.contains(',') {
            return Err(TransportError::InvalidConfiguration {
                reason:
                    "one resource ID must be nonempty and cannot contain Java's comma delimiter",
            });
        }
        Ok(Self {
            branch_type,
            resource_id: resource_id.into(),
        })
    }
    /// 返回键的本地分支模式；原生RegisterRM不另外编码此字段。
    pub const fn branch_type(&self) -> BranchType {
        self.branch_type
    }
    /// 返回精确资源ID借用；调用方负责按数据分类使用，不放入日志。
    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }
}
impl std::fmt::Debug for ResourceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceKey")
            .field("branch_type", &self.branch_type)
            .field("resource_id_bytes", &self.resource_id.len())
            .finish()
    }
}
