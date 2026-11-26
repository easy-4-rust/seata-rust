//! 显式来源迁移、异常行和实际旧读取器边界；不承诺混合版本写入。
use super::{callback_peer, fixture, recovery, rm_fixture};
use seata_db_mysql::{DbError, MySqlRecoveryError};
use seata_protocol::{BranchStatus, ResultCode};
use seata_rm::{
    BranchPhase, BranchRecordState, BranchResource, FileBranchStore, JournalOptions,
    ResourceCatalog, ResourceManager,
};
use std::{path::PathBuf, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn legacy_known_callback_works_but_missing_ownership_requires_explicit_migration() {
    let (mut admin, adapter) = recovery::empty().await;
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let key = adapter.key().clone();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let store = rm_fixture::journal().await;
    let rm = ResourceManager::builder(
        rm_fixture::settings(listener.local_addr().unwrap()),
        rm_fixture::registration(),
        catalog,
        store.clone(),
    )
    .build()
    .unwrap();
    let mut socket = callback_peer::registered(&listener, &key).await;
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "rollback:xid",
            42,
            -101,
            None,
            BranchPhase::Rollback
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "callback:upgrade",
            43,
            -102,
            None,
            BranchPhase::Rollback
        )
        .await,
        (
            ResultCode::Failed,
            BranchStatus::PhaseTwoRollbackFailedRetryable
        )
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    let identity =
        seata_rm::BranchIdentity::new("callback:upgrade".into(), 43, key.clone(), None).unwrap();
    assert_eq!(
        store
            .lease(identity.clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    sqlx::raw_sql(include_str!(
        "../../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "callback:upgrade",
            43,
            -103,
            None,
            BranchPhase::Rollback
        )
        .await,
        (ResultCode::Success, BranchStatus::PhaseTwoRollbacked)
    );
    assert_eq!(
        store
            .lease(identity, BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    // 迁移前完成的原登记行仍为 version1/origin1，未知来源新行为 version2/origin2。
    let facts:Vec<(Vec<u8>,u16,u8,u8,u8)>=sqlx::query_as("SELECT xid,schema_version,registration_origin,phase,terminal FROM seata_rs_at_branch_v1 ORDER BY xid").fetch_all(&mut admin).await.unwrap();
    assert_eq!(
        facts,
        vec![
            (b"callback:upgrade".to_vec(), 2, 2, 2, 1),
            (b"rollback:xid".to_vec(), 1, 1, 2, 1)
        ]
    );
    let before = recovery::state(&mut admin).await;
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "callback:upgrade",
            43,
            -104,
            Some("wrong-app"),
            BranchPhase::Rollback
        )
        .await
        .0,
        ResultCode::Failed
    );
    assert_eq!(
        callback_peer::callback(
            &mut socket,
            &key,
            "callback:upgrade",
            43,
            -105,
            None,
            BranchPhase::Commit
        )
        .await
        .0,
        ResultCode::Failed
    );
    assert_eq!(recovery::state(&mut admin).await, before);
    fixture::save(
        "callback-explicit-migration.json",
        &serde_json::json!({"legacy_known_completed":true,"legacy_missing":"Pending","after_explicit_002":"Completed","rows":facts,"conflicts_preserved":true}),
    );
    rm.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn malformed_origin_version_phase_lock_knowledge_and_default_are_rejected_without_undo() {
    let (mut admin, adapter) = recovery::empty().await;
    sqlx::raw_sql(include_str!(
        "../../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    for (case, version, origin, phase, terminal, lock) in [
        ("zero-origin", 2, 0, 0, 0, Some("undo_work:1")),
        ("unknown-origin", 2, 3, 0, 0, Some("undo_work:1")),
        ("old-version-callback", 1, 2, 2, 0, None),
        ("future-version", 3, 1, 0, 0, Some("undo_work:1")),
        ("unbound-callback", 2, 2, 0, 0, None),
        ("callback-claimed-lock", 2, 2, 2, 0, Some("undo_work:1")),
        ("invalid-phase", 2, 1, 3, 0, Some("undo_work:1")),
        ("invalid-terminal", 2, 1, 2, 2, Some("undo_work:1")),
        ("terminal-commit", 2, 1, 1, 1, Some("undo_work:1")),
    ] {
        sqlx::query("UPDATE seata_rs_at_branch_v1 SET schema_version=?,registration_origin=?,phase=?,terminal=?,lock_key=?").bind(version as u16).bind(origin as u8).bind(phase as u8).bind(terminal as u8).bind(lock.map(str::as_bytes)).execute(&mut admin).await.unwrap();
        assert!(adapter.enroll(&receipt).await.is_err(), "{case}");
        assert_eq!(fixture::count(&mut admin, "undo_log").await, 0, "{case}");
        let state:(u16,u8,u8,u8,Option<Vec<u8>>)=sqlx::query_as("SELECT schema_version,registration_origin,phase,terminal,lock_key FROM seata_rs_at_branch_v1").fetch_one(&mut admin).await.unwrap();
        assert_eq!(
            state,
            (
                version as u16,
                origin as u8,
                phase as u8,
                terminal as u8,
                lock.map(|v| v.as_bytes().to_vec())
            ),
            "{case}"
        );
    }
    sqlx::query("UPDATE seata_rs_at_branch_v1 SET schema_version=2,registration_origin=1,phase=0,terminal=0,lock_key=?").bind(b"undo_work:1".as_slice()).execute(&mut admin).await.unwrap();
    for mutation in [
        "ALTER TABLE seata_rs_at_branch_v1 ALTER COLUMN registration_origin SET DEFAULT 2",
        "ALTER TABLE seata_rs_at_branch_v1 ALTER COLUMN registration_origin DROP DEFAULT",
    ] {
        sqlx::query(mutation).execute(&mut admin).await.unwrap();
        assert!(matches!(
            adapter.enroll(&receipt).await,
            Err(MySqlRecoveryError::BeforeCommit {
                source: DbError::Unsupported { .. },
                ..
            })
        ));
        assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
    }
    sqlx::query("ALTER TABLE seata_rs_at_branch_v1 ALTER COLUMN registration_origin SET DEFAULT 1")
        .execute(&mut admin)
        .await
        .unwrap();
    adapter.enroll(&receipt).await.unwrap();
    fixture::save(
        "callback-invalid-origin.json",
        &serde_json::json!({"invalid_rows_rejected":9,"invalid_defaults_rejected":2,"undo_rows":0,"valid_default_restored":true}),
    );
}

async fn child(executable: PathBuf, journal: PathBuf, output: PathBuf) -> std::process::Output {
    assert!(executable.is_absolute() && executable.to_string_lossy().starts_with("D:"));
    tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new(executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut process = command
            .env("SEATA_RM_CHILD_JOURNAL", journal)
            .env("SEATA_RM_CHILD_OUT", output)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            if process.try_wait().unwrap().is_some() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                process.kill().unwrap();
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        process.wait_with_output().unwrap()
    })
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn archived_old_reader_rejects_expansion_and_current_child_recovers_original_pending() {
    let (mut admin, adapter) = recovery::empty().await;
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/002_seata_rs_at_registration_origin.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let out = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap())
        .join("callback-reader-children");
    std::fs::create_dir_all(&out).unwrap();
    let journal = out.join("branches.log");
    let open = || {
        FileBranchStore::open(
            journal.clone(),
            JournalOptions::new(16, 1048576, 65536, 16).unwrap(),
        )
    };
    let store = open().await.unwrap();
    store
        .lease(receipt.identity().clone(), BranchPhase::Rollback)
        .await
        .unwrap()
        .mark_pending()
        .await
        .unwrap();
    drop(store);
    let old = child(
        PathBuf::from(std::env::var("SEATA_RM_LEGACY_CHILD_EXE").unwrap()),
        journal.clone(),
        out.join("old.json"),
    )
    .await;
    assert!(!old.status.success());
    assert!(
        String::from_utf8_lossy(&old.stderr).contains("persisted recovery did not complete"),
        "{}",
        String::from_utf8_lossy(&old.stderr)
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    let store = open().await.unwrap();
    assert_eq!(
        store
            .lease(receipt.identity().clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Pending)
    );
    drop(store);
    let current = child(
        PathBuf::from(std::env::var("SEATA_RM_CHILD_EXE").unwrap()),
        journal.clone(),
        out.join("current.json"),
    )
    .await;
    assert!(
        current.status.success(),
        "{}",
        String::from_utf8_lossy(&current.stderr)
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    let store = open().await.unwrap();
    assert_eq!(
        store
            .lease(receipt.identity().clone(), BranchPhase::Rollback)
            .await
            .unwrap()
            .state(),
        Some(BranchRecordState::Completed)
    );
    fixture::save(
        "callback-actual-old-reader.json",
        &serde_json::json!({"old_exit":old.status.code(),"old_stderr":String::from_utf8_lossy(&old.stderr),"new_exit":current.status.code(),"new_child":serde_json::from_slice::<serde_json::Value>(&std::fs::read(out.join("current.json")).unwrap()).unwrap(),"old_pending_preserved":true,"original_version_retained":1}),
    );
}
