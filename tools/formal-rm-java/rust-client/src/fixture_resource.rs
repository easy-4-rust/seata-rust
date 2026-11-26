use seata_rm::{BranchIdentity, BranchKey, BranchPhase, BranchResolution, BranchResource, RmError};
use seata_transport::ResourceKey;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// 原JavaTC联调的内存资源夹具；记录执行尝试数，不能用资源自身去重掩盖RM重复apply。
/// 对应 Java: AbstractRMHandler的ResourceManager合约验证夹具，非SQL/Undo生产适配器。
pub struct FixtureResource {
    key: ResourceKey,
    application_data: String,
    effects: Mutex<HashMap<BranchKey, BranchPhase>>,
    apply_calls: AtomicU64,
    recover_calls: AtomicU64,
}
impl FixtureResource {
    /// 参数为固定公共资源身份及原应用数据；返回空效果夹具。
    pub fn new(key: ResourceKey, application_data: String) -> Self {
        Self {
            key,
            application_data,
            effects: Mutex::new(HashMap::new()),
            apply_calls: AtomicU64::new(0),
            recover_calls: AtomicU64::new(0),
        }
    }
    /// 返回原始apply调用次数，重复调用也计数。
    pub fn apply_calls(&self) -> u64 {
        self.apply_calls.load(Ordering::SeqCst)
    }
    /// 返回恢复查询次数，不把查询计作再次执行。
    pub fn recover_calls(&self) -> u64 {
        self.recover_calls.load(Ordering::SeqCst)
    }
    /// 返回已记录的效果数，锁中毒保持失败。
    pub fn effect_count(&self) -> Result<usize, RmError> {
        Ok(self
            .effects
            .lock()
            .map_err(|_| RmError::Unavailable {
                reason: "fixture effect state poisoned",
            })?
            .len())
    }
}
impl std::fmt::Debug for FixtureResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixtureResource")
            .field("key", &self.key)
            .field("apply_calls", &self.apply_calls())
            .finish_non_exhaustive()
    }
}
impl BranchResource for FixtureResource {
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn apply<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(async move {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            if identity.resource() != &self.key
                || identity.application_data() != Some(self.application_data.as_str())
            {
                return Ok(BranchResolution::Rejected);
            }
            let mut effects = self.effects.lock().map_err(|_| RmError::Unavailable {
                reason: "fixture effect state poisoned",
            })?;
            if effects.contains_key(&identity.key()) {
                return Err(RmError::Unavailable {
                    reason: "fixture detected repeated raw apply",
                });
            }
            effects.insert(identity.key(), phase);
            Ok(BranchResolution::Applied)
        })
    }
    fn recover<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(async move {
            self.recover_calls.fetch_add(1, Ordering::SeqCst);
            let effects = self.effects.lock().map_err(|_| RmError::Unavailable {
                reason: "fixture recovery state poisoned",
            })?;
            Ok(match effects.get(&identity.key()) {
                Some(original) if *original == phase => BranchResolution::Applied,
                Some(_) => BranchResolution::Rejected,
                None => BranchResolution::Unknown,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::FixtureResource;
    use seata_protocol::BranchType;
    use seata_rm::{BranchIdentity, BranchPhase, BranchResolution, BranchResource};
    use seata_transport::ResourceKey;
    #[tokio::test]
    #[allow(clippy::unwrap_used)]
    async fn duplicate_raw_apply_is_detected_instead_of_hidden_by_fixture_idempotency() {
        let key = ResourceKey::new(BranchType::At, "fixture".into()).unwrap();
        let resource = FixtureResource::new(key.clone(), "nonce".into());
        let id = BranchIdentity::new("xid".into(), 1, key, Some("nonce".into())).unwrap();
        assert_eq!(
            resource.apply(&id, BranchPhase::Commit).await.unwrap(),
            BranchResolution::Applied
        );
        assert!(resource.apply(&id, BranchPhase::Commit).await.is_err());
        assert_eq!(resource.apply_calls(), 2);
        assert_eq!(resource.effect_count().unwrap(), 1);
        assert_eq!(
            resource.recover(&id, BranchPhase::Commit).await.unwrap(),
            BranchResolution::Applied
        );
    }
    #[tokio::test]
    #[allow(clippy::unwrap_used)]
    async fn missing_effect_stays_unknown_and_wrong_application_data_cannot_apply() {
        let key = ResourceKey::new(BranchType::At, "fixture".into()).unwrap();
        let resource = FixtureResource::new(key.clone(), "nonce".into());
        let id = BranchIdentity::new("xid".into(), 1, key, Some("changed".into())).unwrap();
        assert_eq!(
            resource.recover(&id, BranchPhase::Rollback).await.unwrap(),
            BranchResolution::Unknown
        );
        assert_eq!(
            resource.apply(&id, BranchPhase::Rollback).await.unwrap(),
            BranchResolution::Rejected
        );
        assert_eq!(resource.effect_count().unwrap(), 0);
    }
}
