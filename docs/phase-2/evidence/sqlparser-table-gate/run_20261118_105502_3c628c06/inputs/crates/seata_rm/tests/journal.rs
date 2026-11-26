//! 实际文件、独占锁、重开、串行租约及损坏拒绝；所有测试写入D盘专属目录。
#![allow(clippy::unwrap_used)]
use seata_protocol::BranchType;
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, FileBranchStore, JournalOptions};
use seata_transport::ResourceKey;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(1);
fn path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let folder = PathBuf::from(format!(
        "D:/codex-builds/rm-journal-test-{}-{stamp}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&folder).unwrap();
    folder.join("branches.log")
}
fn options() -> JournalOptions {
    JournalOptions::new(8, 65536, 4096, 8).unwrap()
}
fn branch(id: i64) -> BranchIdentity {
    BranchIdentity::new(
        "tc:9007199254740993".into(),
        id,
        ResourceKey::new(BranchType::At, "mysql://exact-resource".into()).unwrap(),
        Some("{\"nonce\":\"原值\"}".into()),
    )
    .unwrap()
}

#[test]
fn branch_identity_rejects_ambiguous_or_invalid_fields_and_redacts_debug() {
    let value = branch(9007199254740995);
    assert_eq!(value.xid(), "tc:9007199254740993");
    assert_eq!(value.branch_id(), 9007199254740995);
    assert_eq!(value.application_data(), Some("{\"nonce\":\"原值\"}"));
    assert!(!format!("{value:?}").contains("exact-resource"));
    assert!(!format!("{value:?}").contains("原值"));
    let key = value.resource().clone();
    for xid in ["", " xid", "xid\n"] {
        assert!(BranchIdentity::new(xid.into(), 1, key.clone(), None).is_err());
    }
    for id in [0, -1] {
        assert!(BranchIdentity::new("xid".into(), id, key.clone(), None).is_err());
    }
    assert!(BranchIdentity::new("xid".into(), 1, key, Some(String::new())).is_err());
    assert!(JournalOptions::new(0, 65536, 4096, 1).is_err());
    assert!(JournalOptions::new(1, 16, 4096, 1).is_err());
    assert!(JournalOptions::new(1, 65536, 4096, 0).is_err());
}

#[tokio::test]
async fn pending_and_completed_identity_survive_actual_close_and_reopen() {
    let file = path();
    let original = branch(9007199254740995);
    {
        let store = FileBranchStore::open(file.clone(), options())
            .await
            .unwrap();
        let mut lease = store
            .lease(original.clone(), BranchPhase::Commit)
            .await
            .unwrap();
        assert_eq!(lease.state(), None);
        lease.mark_pending().await.unwrap();
        assert_eq!(lease.state(), Some(BranchRecordState::Pending));
        lease.mark_completed().await.unwrap();
        assert_eq!(lease.state(), Some(BranchRecordState::Completed));
    }
    let store = FileBranchStore::open(file, options()).await.unwrap();
    let lease = store
        .lease(original.clone(), BranchPhase::Commit)
        .await
        .unwrap();
    assert_eq!(lease.state(), Some(BranchRecordState::Completed));
    assert_eq!(lease.identity(), &original);
}

#[tokio::test]
async fn cancellation_or_owner_drop_leaves_durable_pending_instead_of_retry_permission() {
    let file = path();
    {
        let store = FileBranchStore::open(file.clone(), options())
            .await
            .unwrap();
        let mut lease = store.lease(branch(1), BranchPhase::Rollback).await.unwrap();
        lease.mark_pending().await.unwrap();
    }
    let store = FileBranchStore::open(file, options()).await.unwrap();
    let mut lease = store.lease(branch(1), BranchPhase::Rollback).await.unwrap();
    assert_eq!(lease.state(), Some(BranchRecordState::Pending));
    lease.mark_retry_safe().await.unwrap();
    lease.mark_pending().await.unwrap();
    lease.mark_rejected().await.unwrap();
    assert_eq!(lease.state(), Some(BranchRecordState::Rejected));
    assert!(lease.mark_pending().await.is_err());
}

#[tokio::test]
async fn an_independent_owner_cannot_open_the_same_journal_while_it_is_owned() {
    let file = path();
    let first = FileBranchStore::open(file.clone(), options())
        .await
        .unwrap();
    assert!(FileBranchStore::open(file.clone(), options())
        .await
        .is_err());
    drop(first);
    assert!(FileBranchStore::open(file, options()).await.is_ok());
}

#[tokio::test]
async fn store_shared_leases_serialize_dispatchers_and_cancelled_waiters_release_budget() {
    let store = FileBranchStore::open(path(), options()).await.unwrap();
    let mut first = store.lease(branch(1), BranchPhase::Commit).await.unwrap();
    first.mark_pending().await.unwrap();
    let cloned = store.clone();
    let second = tokio::spawn(async move { cloned.lease(branch(1), BranchPhase::Commit).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!second.is_finished());
    second.abort();
    assert!(second.await.unwrap_err().is_cancelled());
    first.mark_completed().await.unwrap();
    drop(first);
    let next = tokio::time::timeout(
        Duration::from_secs(1),
        store.lease(branch(1), BranchPhase::Commit),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(next.state(), Some(BranchRecordState::Completed));
}

#[tokio::test]
async fn original_identity_and_phase_cannot_be_changed_even_after_restart() {
    let file = path();
    {
        let store = FileBranchStore::open(file.clone(), options())
            .await
            .unwrap();
        let mut lease = store.lease(branch(1), BranchPhase::Commit).await.unwrap();
        lease.mark_pending().await.unwrap();
        lease.mark_completed().await.unwrap();
    }
    let store = FileBranchStore::open(file, options()).await.unwrap();
    assert!(store.lease(branch(1), BranchPhase::Rollback).await.is_err());
    for value in [
        BranchIdentity::new(
            branch(1).xid().into(),
            1,
            ResourceKey::new(BranchType::Xa, "mysql://exact-resource".into()).unwrap(),
            branch(1).application_data().map(str::to_owned),
        )
        .unwrap(),
        BranchIdentity::new(
            branch(1).xid().into(),
            1,
            branch(1).resource().clone(),
            Some("changed".into()),
        )
        .unwrap(),
    ] {
        assert!(store.lease(value, BranchPhase::Commit).await.is_err());
    }
}

#[tokio::test]
async fn completed_rows_are_not_evicted_and_repeated_completion_adds_no_record() {
    let file = path();
    let opts = JournalOptions::new(1, 65536, 4096, 2).unwrap();
    let store = FileBranchStore::open(file.clone(), opts).await.unwrap();
    {
        let mut lease = store.lease(branch(1), BranchPhase::Commit).await.unwrap();
        assert!(lease.mark_completed().await.is_err());
        lease.mark_pending().await.unwrap();
        lease.mark_completed().await.unwrap();
        let size = std::fs::metadata(&file).unwrap().len();
        lease.mark_completed().await.unwrap();
        assert_eq!(std::fs::metadata(&file).unwrap().len(), size);
    }
    let mut other = store.lease(branch(2), BranchPhase::Commit).await.unwrap();
    assert!(other.mark_pending().await.is_err());
    drop(other);
    drop(store);
    let reopened = FileBranchStore::open(file, opts).await.unwrap();
    assert_eq!(
        reopened
            .lease(branch(1), BranchPhase::Commit)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
}

#[tokio::test]
async fn record_and_file_budgets_fail_before_writing_or_acknowledging_completion() {
    let file = path();
    let store = FileBranchStore::open(file.clone(), JournalOptions::new(1, 256, 128, 2).unwrap())
        .await
        .unwrap();
    let large = BranchIdentity::new(
        "xid".into(),
        1,
        branch(1).resource().clone(),
        Some("x".repeat(512)),
    )
    .unwrap();
    let mut lease = store.lease(large, BranchPhase::Commit).await.unwrap();
    assert!(lease.mark_pending().await.is_err());
    assert_eq!(std::fs::metadata(&file).unwrap().len(), 8);
    drop(lease);
    drop(store);
    // 此身份编码的两条记录可放进200字节；用160字节并验证预算前提，不能把错误夹具当实现缺陷。
    let file = path();
    let store = FileBranchStore::open(file.clone(), JournalOptions::new(1, 160, 128, 2).unwrap())
        .await
        .unwrap();
    let mut lease = store.lease(branch(1), BranchPhase::Commit).await.unwrap();
    lease.mark_pending().await.unwrap();
    let size = std::fs::metadata(&file).unwrap().len();
    assert!(size * 2 - 8 > 160);
    assert!(lease.mark_completed().await.is_err());
    assert_eq!(lease.state(), Some(BranchRecordState::Pending));
    assert_eq!(std::fs::metadata(&file).unwrap().len(), size);
}

#[tokio::test]
async fn damaged_version_checksum_or_partial_tail_is_never_silently_truncated() {
    let source = path();
    {
        let store = FileBranchStore::open(source.clone(), options())
            .await
            .unwrap();
        let mut lease = store.lease(branch(1), BranchPhase::Commit).await.unwrap();
        lease.mark_pending().await.unwrap();
    }
    let original = std::fs::read(&source).unwrap();
    for mode in 0..4 {
        let file = path();
        let mut bytes = original.clone();
        match mode {
            0 => bytes[0] ^= 1,
            1 => {
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
            }
            2 => {
                bytes.pop();
            }
            _ => bytes.extend_from_slice(&[0, 0]),
        }
        std::fs::write(&file, &bytes).unwrap();
        assert!(FileBranchStore::open(file.clone(), options())
            .await
            .is_err());
        assert_eq!(std::fs::read(file).unwrap(), bytes);
    }
}

#[test]
fn cancelled_disk_operation_keeps_its_branch_and_budget_until_the_worker_really_finishes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let store = FileBranchStore::open(path(), JournalOptions::new(2, 65536, 4096, 1).unwrap())
            .await
            .unwrap();
        let mut lease = store.lease(branch(1), BranchPhase::Commit).await.unwrap();
        lease.mark_pending().await.unwrap();
        let (release, held) = std::sync::mpsc::channel();
        let (started, ready) = std::sync::mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            held.recv().unwrap();
        });
        ready.recv().unwrap();
        // 阻塞唯一磁盘工作线程，精确把旧状态变更停在入锁前；取消Future不会取消spawn_blocking。
        let mut mutation = Box::pin(lease.mark_retry_safe());
        std::future::poll_fn(|cx| {
            assert!(mutation.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(mutation);
        drop(lease);
        let mut next = Box::pin(store.lease(branch(2), BranchPhase::Commit));
        let reserved = std::future::poll_fn(|cx| {
            let outcome = matches!(
                next.as_mut().poll(cx),
                std::task::Poll::Ready(Err(seata_rm::RmError::Capacity { .. }))
            );
            std::task::Poll::Ready(outcome)
        })
        .await;
        drop(next);
        release.send(()).unwrap();
        blocker.await.unwrap();
        assert!(
            reserved,
            "cancelled disk work released its capacity/branch fence before completion"
        );
        // 已释放后台线程后，旧写入完成并释放保留预算；不依靠任意时间sleep判断。
        loop {
            match store.lease(branch(1), BranchPhase::Commit).await {
                Ok(next) => {
                    assert_eq!(next.state(), Some(BranchRecordState::RetrySafe));
                    break;
                }
                Err(seata_rm::RmError::Capacity { .. }) => tokio::task::yield_now().await,
                Err(error) => panic!("unexpected journal error: {error}"),
            }
        }
    });
}

#[tokio::test]
async fn checksummed_but_invalid_sequence_state_and_identity_cannot_bypass_replay_validation() {
    let file = path();
    {
        let store = FileBranchStore::open(file.clone(), options())
            .await
            .unwrap();
        let mut lease = store.lease(branch(1), BranchPhase::Commit).await.unwrap();
        lease.mark_pending().await.unwrap();
    }
    let original = std::fs::read(file).unwrap();
    for mode in 0..4 {
        let file = path();
        let mut bytes = original.clone();
        match mode {
            0 => bytes[12..20].copy_from_slice(&2_u64.to_be_bytes()),
            1 => bytes[20] = BranchRecordState::Completed as u8,
            2 => bytes[20] = 255,
            _ => {
                bytes[21..23].copy_from_slice(&1_u16.to_be_bytes());
            }
        }
        let end = bytes.len() - 4;
        let checksum = crc32fast::hash(&bytes[12..end]);
        bytes[end..].copy_from_slice(&checksum.to_be_bytes());
        std::fs::write(&file, &bytes).unwrap();
        assert!(FileBranchStore::open(file.clone(), options())
            .await
            .is_err());
        assert_eq!(std::fs::read(file).unwrap(), bytes);
    }
}

#[tokio::test]
async fn empty_existing_file_is_corruption_and_not_a_new_journal() {
    let file = path();
    std::fs::write(&file, b"").unwrap();
    assert!(FileBranchStore::open(file.clone(), options())
        .await
        .is_err());
    assert!(std::fs::read(file).unwrap().is_empty());
}
