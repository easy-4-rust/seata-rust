use crate::ResourceKey;
use std::sync::Arc;
/// 一个修订号下的不可变有序资源快照，跨重连共享且不泄露标识到Debug。
/// 对应 Java: AbstractResourceManager#getManagedResources 的受界快照职责。
#[derive(Debug, Clone)]
pub struct ResourceRegistrySnapshot {
    pub(crate) revision: u64,
    pub(crate) resources: Arc<[ResourceKey]>,
}
impl ResourceRegistrySnapshot {
    /// 返回该集合的单调修订号；重复insert不改变它。
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// 返回保持插入顺序的完整typed资源键切片。
    pub fn resources(&self) -> &[ResourceKey] {
        &self.resources
    }
}
