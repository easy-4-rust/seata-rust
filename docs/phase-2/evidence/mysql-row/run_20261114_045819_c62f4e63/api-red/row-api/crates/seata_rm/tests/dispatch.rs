//! 观察资源动作次数、恢复查询和持久状态，避免把一次成功响应等同于资源事实。
#![allow(clippy::unwrap_used)]
use seata_protocol::{BranchStatus, BranchType};
use seata_rm::{
    BranchDispatcher, BranchIdentity, BranchPhase, BranchRecordState, BranchResolution,
    BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, RmError,
};
use seata_transport::ResourceKey;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Debug)]
struct Resource {
    key: ResourceKey,
    applied: AtomicU64,
    recovered: AtomicU64,
    apply_answer: Mutex<BranchResolution>,
    recover_answer: Mutex<BranchResolution>,
    stall: bool,
    fail: bool,
    entered: Notify,
}
impl Resource {
    fn new(kind: BranchType, name: &str) -> Arc<Self> {
        Arc::new(Self {
            key: ResourceKey::new(kind, name.into()).unwrap(),
            applied: AtomicU64::new(0),
            recovered: AtomicU64::new(0),
            apply_answer: Mutex::new(BranchResolution::Applied),
            recover_answer: Mutex::new(BranchResolution::Unknown),
            stall: false,
            fail: false,
            entered: Notify::new(),
        })
    }
}
impl BranchResource for Resource {
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn apply<'a>(
        &'a self,
        _identity: &'a BranchIdentity,
        _phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        self.applied.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if self.fail {
            Box::pin(async {
                Err(RmError::Unavailable {
                    reason: "fixture apply failed after possible effect",
                })
            })
        } else if self.stall {
            Box::pin(std::future::pending())
        } else {
            let answer = *self.apply_answer.lock().unwrap();
            Box::pin(async move { Ok(answer) })
        }
    }
    fn recover<'a>(
        &'a self,
        _identity: &'a BranchIdentity,
        _phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        self.recovered.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Box::pin(async {
                Err(RmError::Unavailable {
                    reason: "fixture recovery unavailable",
                })
            });
        }
        let answer = *self.recover_answer.lock().unwrap();
        Box::pin(async move { Ok(answer) })
    }
}
fn identity(resource: &ResourceKey, id: i64) -> BranchIdentity {
    BranchIdentity::new(
        "tc:9007199254740993".into(),
        id,
        resource.clone(),
        Some("original-nonce".into()),
    )
    .unwrap()
}
async fn store() -> FileBranchStore {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let folder = format!(
        "D:/codex-builds/rm-dispatch-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    );
    std::fs::create_dir_all(&folder).unwrap();
    FileBranchStore::open(
        std::path::PathBuf::from(folder).join("branches.log"),
        JournalOptions::new(8, 65536, 4096, 8).unwrap(),
    )
    .await
    .unwrap()
}
fn catalog(resource: Arc<Resource>) -> ResourceCatalog {
    let catalog = ResourceCatalog::new(4, 4096).unwrap();
    catalog.register(resource).unwrap();
    catalog
}

#[tokio::test]
async fn exact_duplicate_uses_completed_record_but_identity_or_phase_changes_do_not_execute() {
    let resource = Resource::new(BranchType::At, "mysql://one");
    let store = store().await;
    let dispatcher = BranchDispatcher::new(catalog(resource.clone()), store.clone());
    let value = identity(&resource.key, 9007199254740995);
    for _ in 0..2 {
        assert_eq!(
            dispatcher
                .dispatch(value.clone(), BranchPhase::Commit)
                .await
                .unwrap(),
            BranchStatus::PhaseTwoCommitted
        );
    }
    assert_eq!(resource.applied.load(Ordering::SeqCst), 1);
    assert_eq!(resource.recovered.load(Ordering::SeqCst), 0);
    assert!(dispatcher
        .dispatch(value.clone(), BranchPhase::Rollback)
        .await
        .is_err());
    let changed = BranchIdentity::new(
        value.xid().into(),
        value.branch_id(),
        value.resource().clone(),
        Some("changed".into()),
    )
    .unwrap();
    assert!(dispatcher
        .dispatch(changed, BranchPhase::Commit)
        .await
        .is_err());
    assert_eq!(resource.applied.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn pending_unknown_is_queried_without_action_and_authoritative_applied_finishes_without_replay(
) {
    let resource = Resource::new(BranchType::At, "mysql://one");
    let store = store().await;
    let value = identity(&resource.key, 1);
    {
        let mut lease = store
            .lease(value.clone(), BranchPhase::Rollback)
            .await
            .unwrap();
        lease.mark_pending().await.unwrap();
    }
    let dispatcher = BranchDispatcher::new(catalog(resource.clone()), store.clone());
    assert_eq!(
        dispatcher
            .dispatch(value.clone(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchStatus::PhaseTwoRollbackFailedRetryable
    );
    assert_eq!(resource.applied.load(Ordering::SeqCst), 0);
    *resource.recover_answer.lock().unwrap() = BranchResolution::Applied;
    assert_eq!(
        dispatcher
            .dispatch(value.clone(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchStatus::PhaseTwoRollbacked
    );
    assert_eq!(resource.applied.load(Ordering::SeqCst), 0);
    assert_eq!(resource.recovered.load(Ordering::SeqCst), 2);
    assert_eq!(
        store
            .lease(value, BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
}

#[tokio::test]
async fn authoritative_no_effect_and_rejection_have_different_retry_behavior() {
    let resource = Resource::new(BranchType::At, "mysql://one");
    *resource.apply_answer.lock().unwrap() = BranchResolution::NotApplied;
    let store = store().await;
    let dispatcher = BranchDispatcher::new(catalog(resource.clone()), store.clone());
    let value = identity(&resource.key, 1);
    assert_eq!(
        dispatcher
            .dispatch(value.clone(), BranchPhase::Commit)
            .await
            .unwrap(),
        BranchStatus::PhaseTwoCommitFailedRetryable
    );
    assert_eq!(
        store
            .lease(value.clone(), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::RetrySafe)
    );
    *resource.apply_answer.lock().unwrap() = BranchResolution::Rejected;
    for _ in 0..2 {
        assert_eq!(
            dispatcher
                .dispatch(value.clone(), BranchPhase::Commit)
                .await
                .unwrap(),
            BranchStatus::PhaseTwoCommitFailedUnretryable
        );
    }
    assert_eq!(resource.applied.load(Ordering::SeqCst), 2);
    assert_eq!(resource.recovered.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancelled_apply_leaves_pending_and_a_separate_dispatcher_queries_before_any_new_action() {
    let resource = Arc::new(Resource {
        stall: true,
        ..Arc::try_unwrap(Resource::new(BranchType::At, "mysql://one")).unwrap()
    });
    *resource.recover_answer.lock().unwrap() = BranchResolution::Applied;
    let store = store().await;
    let catalog = catalog(resource.clone());
    let dispatcher = BranchDispatcher::new(catalog.clone(), store.clone());
    let value = identity(&resource.key, 1);
    let moved = value.clone();
    let task = tokio::spawn(async move { dispatcher.dispatch(moved, BranchPhase::Commit).await });
    resource.entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        store
            .lease(value.clone(), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    assert_eq!(
        BranchDispatcher::new(catalog, store)
            .dispatch(value, BranchPhase::Commit)
            .await
            .unwrap(),
        BranchStatus::PhaseTwoCommitted
    );
    assert_eq!(resource.applied.load(Ordering::SeqCst), 1);
    assert_eq!(resource.recovered.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn simultaneous_dispatchers_share_one_branch_lease_and_never_repeat_a_confirmed_action() {
    let resource = Resource::new(BranchType::At, "mysql://one");
    let store = store().await;
    let catalog = catalog(resource.clone());
    let value = identity(&resource.key, 1);
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let dispatcher = BranchDispatcher::new(catalog.clone(), store.clone());
        let value = value.clone();
        tasks.spawn(async move { dispatcher.dispatch(value, BranchPhase::Commit).await });
    }
    while let Some(result) = tasks.join_next().await {
        assert_eq!(result.unwrap().unwrap(), BranchStatus::PhaseTwoCommitted);
    }
    assert_eq!(resource.applied.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn catalog_never_announces_a_replacement_or_an_unhandled_resource() {
    let resource = Resource::new(BranchType::At, "mysql://one");
    let catalog = ResourceCatalog::new(1, 4096).unwrap();
    assert!(catalog.register(resource.clone()).unwrap());
    assert!(!catalog.register(resource.clone()).unwrap());
    assert!(catalog
        .register(Resource::new(BranchType::At, "mysql://one"))
        .is_err());
    assert!(catalog
        .register(Resource::new(BranchType::Xa, "mysql://one"))
        .is_err());
    assert_eq!(catalog.snapshot().unwrap().revision(), 1);
    assert_eq!(
        catalog.snapshot().unwrap().resources(),
        std::slice::from_ref(&resource.key)
    );
    let dispatcher = BranchDispatcher::new(catalog, store().await);
    assert!(dispatcher
        .dispatch(
            identity(
                &ResourceKey::new(BranchType::Xa, "mysql://one".into()).unwrap(),
                1
            ),
            BranchPhase::Commit
        )
        .await
        .is_err());
    assert_eq!(resource.applied.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn adapter_errors_keep_pending_and_recovery_errors_do_not_allow_another_action() {
    let resource = Arc::new(Resource {
        fail: true,
        ..Arc::try_unwrap(Resource::new(BranchType::At, "mysql://one")).unwrap()
    });
    let store = store().await;
    let dispatcher = BranchDispatcher::new(catalog(resource.clone()), store.clone());
    let value = identity(&resource.key, 1);
    assert!(dispatcher
        .dispatch(value.clone(), BranchPhase::Commit)
        .await
        .is_err());
    assert_eq!(
        store
            .lease(value.clone(), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    assert!(dispatcher
        .dispatch(value, BranchPhase::Commit)
        .await
        .is_err());
    assert_eq!(resource.applied.load(Ordering::SeqCst), 1);
    assert_eq!(resource.recovered.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn only_authoritative_recovery_no_effect_permits_a_new_attempt() {
    let resource = Resource::new(BranchType::At, "mysql://one");
    let store = store().await;
    let value = identity(&resource.key, 1);
    {
        let mut lease = store
            .lease(value.clone(), BranchPhase::Rollback)
            .await
            .unwrap();
        lease.mark_pending().await.unwrap();
    }
    *resource.recover_answer.lock().unwrap() = BranchResolution::NotApplied;
    assert_eq!(
        BranchDispatcher::new(catalog(resource.clone()), store)
            .dispatch(value, BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchStatus::PhaseTwoRollbacked
    );
    assert_eq!(resource.recovered.load(Ordering::SeqCst), 1);
    assert_eq!(resource.applied.load(Ordering::SeqCst), 1);
}
