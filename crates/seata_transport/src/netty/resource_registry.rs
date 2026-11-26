use crate::{ResourceKey, ResourceRegistrySnapshot, TransportError};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
/// 进程内有界共享资源缓存，checked修订和通知使新资源不能借旧ready记录被发布。
/// 对应 Java: AbstractResourceManager的resourceCache及RegisterCachedResources职责。
#[derive(Clone)]
pub struct ResourceRegistry {
    inner: Arc<RegistryState>,
}
/// ResourceRegistry内部伴随预算和同步状态；不跨await持锁。
/// 对应 Java: AbstractResourceManager资源缓存内部状态。
struct RegistryState {
    maximum_entries: usize,
    maximum_id_bytes: usize,
    snapshot: Mutex<ResourceRegistrySnapshot>,
    changed: watch::Sender<u64>,
}
impl ResourceRegistry {
    /// 参数是资源数上限(1..4096)和单ID字节上限；返回空的、修订为零的有界缓存。
    pub fn new(maximum_entries: usize, maximum_id_bytes: usize) -> Result<Self, TransportError> {
        if maximum_entries == 0
            || maximum_entries > 4096
            || maximum_id_bytes == 0
            || maximum_id_bytes > i32::MAX as usize
        {
            return Err(TransportError::InvalidConfiguration {
                reason:
                    "resource registry requires 1..4096 entries and positive signed-int ID bytes",
            });
        }
        let (changed, _) = watch::channel(0);
        Ok(Self {
            inner: Arc::new(RegistryState {
                maximum_entries,
                maximum_id_bytes,
                snapshot: Mutex::new(ResourceRegistrySnapshot {
                    revision: 0,
                    resources: Arc::from([]),
                }),
                changed,
            }),
        })
    }
    /// 参数是精确typed键；新插入返回true并原子推进修订，重复返回false，预算/毒锁错误不改写缓存。
    pub fn insert(&self, key: ResourceKey) -> Result<bool, TransportError> {
        let mut snapshot =
            self.inner
                .snapshot
                .lock()
                .map_err(|_| TransportError::InternalState {
                    reason: "resource registry lock poisoned",
                })?;
        if snapshot.resources.contains(&key) {
            return Ok(false);
        }
        if snapshot.resources.len() >= self.inner.maximum_entries
            || key.resource_id().len() > self.inner.maximum_id_bytes
        {
            return Err(TransportError::InvalidConfiguration {
                reason: "resource registry entry or ID byte budget exceeded",
            });
        }
        let revision = snapshot
            .revision
            .checked_add(1)
            .ok_or(TransportError::InternalState {
                reason: "resource registry revision exhausted",
            })?;
        let mut resources = snapshot.resources.to_vec();
        resources.push(key);
        snapshot.resources = resources.into();
        snapshot.revision = revision;
        // 通知在同一同步顺序中更新，防止两个insert把watch值逆序发布；没有借用通知跨await。
        self.inner.changed.send_replace(revision);
        Ok(true)
    }
    /// 返回当前不可变快照；毒锁保持失败封闭，不能伪造旧集合为最新资源。
    pub fn snapshot(&self) -> Result<ResourceRegistrySnapshot, TransportError> {
        self.inner
            .snapshot
            .lock()
            .map(|value| value.clone())
            .map_err(|_| TransportError::InternalState {
                reason: "resource registry lock poisoned",
            })
    }
    /// 返回资源变更订阅；只把通知用作唤醒，权威修订仍从snapshot读取。
    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.changed.subscribe()
    }
}
impl std::fmt::Debug for ResourceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceRegistry")
            .field("maximum_entries", &self.inner.maximum_entries)
            .field("maximum_id_bytes", &self.inner.maximum_id_bytes)
            .field("snapshot", &self.snapshot())
            .finish()
    }
}
