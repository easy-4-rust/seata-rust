use crate::{BranchResource, RmError};
use seata_transport::{ResourceKey, ResourceRegistry, ResourceRegistrySnapshot};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

/// 精确资源执行器目录，先发布处理器再推进原生注册修订；不允许替换已有执行器。
/// 对应 Java: DefaultResourceManager分派和AbstractResourceManager#resourceCache。
#[derive(Clone)]
pub struct ResourceCatalog {
    inner: Arc<CatalogInner>,
}
/// 目录内部伴随状态；处理器安装和注册修订按统一同步顺序发布。
/// 对应 Java: AbstractResourceManager#resourceCache及缓存资源注册职责。
struct CatalogInner {
    resources: Mutex<HashMap<ResourceKey, Arc<dyn BranchResource>>>,
    registry: ResourceRegistry,
}
impl ResourceCatalog {
    /// 返回内部原生登记缓存克隆，只供拥有目录的RM网络所有者使用。
    pub(crate) fn registry(&self) -> ResourceRegistry {
        self.inner.registry.clone()
    }
    /// 参数为资源数及ID字节预算；返回空目录与配套注册缓存。
    pub fn new(maximum_entries: usize, maximum_id_bytes: usize) -> Result<Self, RmError> {
        let registry = ResourceRegistry::new(maximum_entries, maximum_id_bytes).map_err(|_| {
            RmError::Invalid {
                reason: "invalid resource catalog budget",
            }
        })?;
        Ok(Self {
            inner: Arc::new(CatalogInner {
                resources: Mutex::new(HashMap::new()),
                registry,
            }),
        })
    }
    /// 参数是稳定身份的执行器；新资源返回true，同一Arc幂等返回false，其他替换拒绝。
    /// 修订公开前处理器已可查；预算错误回滚未发布的处理器。
    pub fn register(&self, resource: Arc<dyn BranchResource>) -> Result<bool, RmError> {
        let key = resource.key().clone();
        let mut resources = self
            .inner
            .resources
            .lock()
            .map_err(|_| RmError::Unavailable {
                reason: "resource catalog poisoned",
            })?;
        if let Some(existing) = resources.get(&key) {
            return if Arc::ptr_eq(existing, &resource) {
                Ok(false)
            } else {
                Err(RmError::IdentityConflict)
            };
        }
        resources.insert(key.clone(), resource);
        match self.inner.registry.insert(key.clone()) {
            Ok(true) => Ok(true),
            Ok(false) => {
                resources.remove(&key);
                Err(RmError::Unavailable {
                    reason: "registration published without catalog handler",
                })
            }
            Err(_) => {
                resources.remove(&key);
                Err(RmError::Capacity {
                    budget: "resource registry",
                })
            }
        }
    }
    /// 返回只读原生注册快照；不暴露可绕过处理器目录的可写注册缓存。
    pub fn snapshot(&self) -> Result<ResourceRegistrySnapshot, RmError> {
        self.inner
            .registry
            .snapshot()
            .map_err(|_| RmError::Unavailable {
                reason: "resource registry poisoned",
            })
    }
    /// 参数为精确模式/ID；返回已安装的执行器，未注册或锁中毒返回错误。
    pub(crate) fn lookup(&self, key: &ResourceKey) -> Result<Arc<dyn BranchResource>, RmError> {
        self.inner
            .resources
            .lock()
            .map_err(|_| RmError::Unavailable {
                reason: "resource catalog poisoned",
            })?
            .get(key)
            .cloned()
            .ok_or(RmError::Unavailable {
                reason: "resource handler not registered",
            })
    }
}
impl fmt::Debug for ResourceCatalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceCatalog")
            .field("snapshot", &self.snapshot())
            .finish_non_exhaustive()
    }
}
