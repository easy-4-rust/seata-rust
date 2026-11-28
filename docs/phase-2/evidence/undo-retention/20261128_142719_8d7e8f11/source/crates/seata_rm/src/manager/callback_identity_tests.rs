#![allow(clippy::unwrap_used)]
use crate::{
    BranchCallbackReceipt, BranchDispatcher, BranchIdentity, BranchPhase, BranchResolution,
    BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, RmError,
};
use seata_protocol::{BranchStatus, BranchType};
use seata_transport::ResourceKey;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
/// 只为测试提供稳定身份，证明分派器不会信任改变原分支键的资源实现。
/// 对应 Java: 无直接对象；Rust 回调稳定身份边界反例。
#[derive(Debug)]
struct CallbackIdentityProbe {
    key: ResourceKey,
    stable: BranchIdentity,
    checks: AtomicUsize,
    actions: AtomicUsize,
}
impl BranchResource for CallbackIdentityProbe {
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn callback_identity<'a>(
        &'a self,
        _receipt: &'a BranchCallbackReceipt,
    ) -> Pin<Box<dyn Future<Output = Result<BranchIdentity, RmError>> + Send + 'a>> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(self.stable.clone()) })
    }
    fn apply<'a>(
        &'a self,
        _identity: &'a BranchIdentity,
        _phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        self.actions.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(BranchResolution::Applied) })
    }
    fn recover<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        self.apply(identity, phase)
    }
}
#[tokio::test]
async fn callback_identity_cannot_change_branch_or_resource_and_is_checked_before_completed_reuse()
{
    for variant in 0..5 {
        let key = ResourceKey::new(BranchType::Tcc, "original-action".into()).unwrap();
        let original = BranchIdentity::new(
            "original-xid".into(),
            1,
            key.clone(),
            Some("original-app".into()),
        )
        .unwrap();
        let stable = BranchIdentity::new(
            if variant == 0 {
                "other-xid"
            } else {
                "original-xid"
            }
            .into(),
            if variant == 1 { 2 } else { 1 },
            ResourceKey::new(
                if variant == 2 {
                    BranchType::At
                } else {
                    BranchType::Tcc
                },
                if variant == 3 {
                    "other-action"
                } else {
                    "original-action"
                }
                .into(),
            )
            .unwrap(),
            Some("original-app".into()),
        )
        .unwrap();
        let resource = Arc::new(CallbackIdentityProbe {
            key,
            stable,
            checks: AtomicUsize::new(0),
            actions: AtomicUsize::new(0),
        });
        let folder = std::path::PathBuf::from(format!(
            "D:/codex-builds/rm-callback-identity-{}-{variant}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let store = FileBranchStore::open(
            folder.join("branches.log"),
            JournalOptions::new(8, 65536, 4096, 8).unwrap(),
        )
        .await
        .unwrap();
        let mut lease = store
            .lease(original.clone(), BranchPhase::Commit)
            .await
            .unwrap();
        lease.mark_pending().await.unwrap();
        lease.mark_completed().await.unwrap();
        drop(lease);
        let catalog = ResourceCatalog::new(4, 32767).unwrap();
        catalog.register(resource.clone()).unwrap();
        let dispatcher = BranchDispatcher::new(catalog, store);
        let updated = BranchIdentity::new(
            original.xid().into(),
            1,
            original.resource().clone(),
            Some("new-app".into()),
        )
        .unwrap();
        let result = dispatcher
            .dispatch_callback(BranchCallbackReceipt::received(
                updated.clone(),
                BranchPhase::Commit,
            ))
            .await;
        if variant == 4 {
            assert_eq!(result.unwrap(), BranchStatus::PhaseTwoCommitted);
        } else {
            assert!(matches!(result, Err(RmError::IdentityConflict)));
        }
        assert_eq!(resource.checks.load(Ordering::SeqCst), 1);
        assert_eq!(resource.actions.load(Ordering::SeqCst), 0);
        assert!(matches!(
            dispatcher.dispatch(updated, BranchPhase::Commit).await,
            Err(RmError::IdentityConflict)
        ));
        assert_eq!(resource.checks.load(Ordering::SeqCst), 1);
    }
}
