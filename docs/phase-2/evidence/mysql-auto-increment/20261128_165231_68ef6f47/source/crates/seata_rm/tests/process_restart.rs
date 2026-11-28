//! 真正结束子进程，在资源效果已同步、RM完成日志尚未写入时恢复；夹具不是SQL/Undo实现。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_protocol::BranchType;
use seata_rm::{
    BranchDispatcher, BranchIdentity, BranchPhase, BranchRecordState, BranchResolution,
    BranchResource, FileBranchStore, JournalOptions, ResourceCatalog, RmError,
};
use seata_transport::ResourceKey;
use std::fs::OpenOptions;
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct DurableFixture {
    key: ResourceKey,
    folder: PathBuf,
    crash_window: bool,
}
struct OwnedChild(Child);
impl std::ops::Deref for OwnedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.0
    }
}
impl std::ops::DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn options() -> JournalOptions {
    JournalOptions::new(8, 65536, 4096, 8).unwrap()
}
fn identity(key: &ResourceKey) -> BranchIdentity {
    BranchIdentity::new(
        "tc:9007199254740993".into(),
        9007199254740995,
        key.clone(),
        Some("进程恢复-原始nonce".into()),
    )
    .unwrap()
}
fn expected(identity: &BranchIdentity, phase: BranchPhase) -> Vec<u8> {
    format!(
        "{}\n{}\n{}\n{:?}\n{}",
        identity.xid(),
        identity.branch_id(),
        identity.resource().resource_id(),
        phase,
        identity.application_data().unwrap()
    )
    .into_bytes()
}
fn audit(folder: &Path, name: &str) -> Result<(), RmError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join(name))?;
    file.write_all(b"1\n")?;
    file.sync_all()?;
    Ok(())
}
impl BranchResource for DurableFixture {
    fn key(&self) -> &ResourceKey {
        &self.key
    }
    fn apply<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(async move {
            audit(&self.folder, "apply.audit")?;
            let mut effect = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(self.folder.join("effect.bin"))?;
            effect.write_all(&expected(identity, phase))?;
            effect.sync_all()?;
            if self.crash_window {
                let mut ready = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(self.folder.join("effect-ready"))?;
                ready.write_all(b"effect durable, no RM completion yet")?;
                ready.sync_all()?;
                // 父进程看到标志后真正终止整个子进程，不能通过Drop清理或直接伪造已恢复状态。
                std::future::pending::<()>().await;
            }
            Ok(BranchResolution::Applied)
        })
    }
    fn recover<'a>(
        &'a self,
        identity: &'a BranchIdentity,
        phase: BranchPhase,
    ) -> Pin<Box<dyn Future<Output = Result<BranchResolution, RmError>> + Send + 'a>> {
        Box::pin(async move {
            audit(&self.folder, "recover.audit")?;
            // 日志独占锁已证明旧夹具进程退出；持久标记绑定完整身份和方向。
            // 这里不根据“缺失行”断言NotApplied；不实现通用数据库提交围栏。
            match std::fs::read(self.folder.join("effect.bin")) {
                Ok(bytes) if bytes == expected(identity, phase) => Ok(BranchResolution::Applied),
                Ok(_) => Ok(BranchResolution::Unknown),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(BranchResolution::Unknown)
                }
                Err(error) => Err(error.into()),
            }
        })
    }
}

#[test]
#[ignore = "parent launches this exact helper in a separately owned process"]
fn journal_process_child() {
    let folder = PathBuf::from(std::env::var_os("RM_RESTART_FOLDER").expect("owned parent folder"));
    let mode = std::env::var("RM_RESTART_MODE").unwrap();
    let phase = match std::env::var("RM_RESTART_PHASE").unwrap().as_str() {
        "Commit" => BranchPhase::Commit,
        "Rollback" => BranchPhase::Rollback,
        _ => panic!("phase"),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let store = FileBranchStore::open(folder.join("branches.log"), options())
            .await
            .unwrap();
        let key = ResourceKey::new(BranchType::At, "mysql://restart-fixture".into()).unwrap();
        let value = identity(&key);
        let initial = store.lease(value.clone(), phase).await.unwrap().state();
        match mode.as_str() {
            "crash" => assert_eq!(initial, None),
            "recover" => assert_eq!(initial, Some(BranchRecordState::Pending)),
            "duplicate" => assert_eq!(initial, Some(BranchRecordState::Completed)),
            _ => panic!("mode"),
        };
        let catalog = ResourceCatalog::new(1, 4096).unwrap();
        catalog
            .register(Arc::new(DurableFixture {
                key,
                folder: folder.clone(),
                crash_window: mode == "crash",
            }))
            .unwrap();
        let status = BranchDispatcher::new(catalog, store.clone())
            .dispatch(value.clone(), phase)
            .await
            .unwrap();
        assert_eq!(
            status.to_java_v1(),
            if phase == BranchPhase::Commit { 5 } else { 8 }
        );
        assert_eq!(
            store.lease(value, phase).await.unwrap().state(),
            Some(BranchRecordState::Completed)
        );
    });
}

fn child(folder: &Path, mode: &str, phase: BranchPhase) -> OwnedChild {
    let stdout = std::fs::File::create(folder.join(format!("{mode}.stdout"))).unwrap();
    let stderr = std::fs::File::create(folder.join(format!("{mode}.stderr"))).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "journal_process_child",
            "--ignored",
            "--nocapture",
        ])
        .env("RM_RESTART_FOLDER", folder)
        .env("RM_RESTART_MODE", mode)
        .env("RM_RESTART_PHASE", format!("{phase:?}"))
        .stdout(stdout)
        .stderr(stderr);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    OwnedChild(command.spawn().unwrap())
}
fn finish(mut child: OwnedChild) {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "restart helper failed: {status}");
            return;
        }
        if started.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("owned restart helper timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn crash_then_recover(phase: BranchPhase) {
    let folder = PathBuf::from(format!(
        "D:/codex-builds/rm-process-{}-{}-{phase:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let mut first = child(&folder, "crash", phase);
    let started = Instant::now();
    while !folder.join("effect-ready").exists() {
        if let Some(status) = first.try_wait().unwrap() {
            panic!("child exited before durable effect: {status}");
        }
        if started.elapsed() > Duration::from_secs(10) {
            first.kill().unwrap();
            first.wait().unwrap();
            panic!("effect not observed");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let locked = runtime
        .block_on(FileBranchStore::open(
            folder.join("branches.log"),
            options(),
        ))
        .is_err();
    first.kill().unwrap();
    let killed = first.wait().unwrap();
    assert!(!killed.success());
    assert!(locked, "live owner lock was bypassed");
    finish(child(&folder, "recover", phase));
    finish(child(&folder, "duplicate", phase));
    assert_eq!(std::fs::read(folder.join("apply.audit")).unwrap(), b"1\n");
    assert_eq!(std::fs::read(folder.join("recover.audit")).unwrap(), b"1\n");
    let key = ResourceKey::new(BranchType::At, "mysql://restart-fixture".into()).unwrap();
    assert_eq!(
        std::fs::read(folder.join("effect.bin")).unwrap(),
        expected(&identity(&key), phase)
    );
    println!(
        "process-restart-evidence={} phase={phase:?} apply=1 recover=1 children=3 terminated=true",
        folder.display()
    );
}
#[test]
fn commit_effect_survives_forced_process_exit_and_two_new_processes_do_not_repeat_it() {
    crash_then_recover(BranchPhase::Commit);
}
#[test]
fn rollback_effect_survives_forced_process_exit_and_two_new_processes_do_not_repeat_it() {
    crash_then_recover(BranchPhase::Rollback);
}
