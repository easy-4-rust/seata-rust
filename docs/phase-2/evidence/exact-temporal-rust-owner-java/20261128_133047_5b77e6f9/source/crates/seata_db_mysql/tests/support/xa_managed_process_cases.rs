//! 独立实际写入进程、原响应截留、强制退出及新 RM/旧日志恢复。
#![allow(clippy::unwrap_used)]
use super::xa_command_proxy;
#[path = "xa_owned_process.rs"]
mod xa_owned_process;
use super::{bootstrap, callback, fixture, idle, peer, registered};
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::{MySqlDataSource, MySqlXaResourceManager};
use seata_protocol::{
    transaction::{BranchCommitRequest, BranchRollbackRequest},
    BranchType, Message, RequestType, ResultCode,
};
use seata_rm::{
    BranchIdentity, BranchPhase, BranchRecordState, BranchResource, FileBranchStore,
    JournalOptions, ResourceCatalog, ResourceManager,
};
use std::{fs::File, path::Path, process::Command, sync::Arc, time::Duration};
use tokio::net::TcpListener;

async fn manager(
    adapter: Arc<MySqlXaResourceManager>,
    endpoint: std::net::SocketAddr,
    store: FileBranchStore,
) -> ResourceManager {
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter).unwrap();
    ResourceManager::builder(
        fixture::pool(endpoint),
        fixture::registration(false),
        catalog,
        store,
    )
    .request_timeout(Duration::from_secs(5))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap()
}
async fn journal(path: &Path) -> FileBranchStore {
    FileBranchStore::open(
        path.to_path_buf(),
        JournalOptions::new(32, 1048576, 65536, 32).unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "only invoked by owned process matrix"]
async fn managed_xa_crash_worker() {
    let stage = std::env::var("SEATA_XA_PROCESS_STAGE").unwrap();
    // 即使父测试被操作系统强制终止，夹具也只在本进程内于 45 秒后自行退出。
    let _watchdog = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(45));
        std::process::exit(91);
    });
    assert!([
        "enlisted",
        "active",
        "prepared_commit",
        "prepared_rollback",
        "commit_ok",
        "rollback_ok",
        "terminal_commit_ok",
        "terminal_rollback_ok",
        "unresponsive"
    ]
    .contains(&stage.as_str()));
    let source = MySqlDataSource::connect(fixture::owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = Arc::new(MySqlXaResourceManager::new(source.clone()).unwrap());
    let store_path = fixture::folder("owner").join("branches.log");
    let store = journal(&store_path).await;
    let rm = manager(
        adapter.clone(),
        std::env::var("SEATA_RM_ENDPOINT").unwrap().parse().unwrap(),
        store,
    )
    .await;
    let xid = format!("xa:process:{stage}");
    let mut scope = adapter
        .register_and_start(&rm.client(), &xid, None)
        .await
        .unwrap();
    let receipt = scope.registered_branch().clone();
    if stage != "enlisted" {
        scope
            .execute(
                &SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(1,42)")
                    .unwrap(),
                &[],
            )
            .await
            .unwrap();
    }
    let scope = if matches!(stage.as_str(), "enlisted" | "active" | "unresponsive") {
        Some(scope)
    } else {
        assert_eq!(
            scope.prepare().await.unwrap().identity(),
            receipt.identity()
        );
        None
    };
    fixture::record(
        "process-checkpoint",
        serde_json::json!({"stage":stage,"xid":receipt.identity().xid(),"branch_id":receipt.identity().branch_id().to_string(),"resource_id":receipt.identity().resource().resource_id(),"journal":store_path,"prepared":scope.is_none()}),
    );
    let owned = (scope, receipt, rm, source, adapter);
    if stage == "unresponsive" {
        loop {
            std::thread::park();
        }
    }
    std::future::pending::<()>().await;
    drop(owned);
}

async fn guard_locked(admin: &mut sqlx::MySqlConnection, id: &BranchIdentity) -> bool {
    sqlx::raw_sql("START TRANSACTION")
        .execute(&mut *admin)
        .await
        .unwrap();
    let result = sqlx::query(
        "SELECT 1 FROM seata_rs_xa_session_guard_v1 WHERE xid=? AND branch_id=? FOR UPDATE NOWAIT",
    )
    .bind(id.xid().as_bytes())
    .bind(id.branch_id())
    .fetch_optional(&mut *admin)
    .await;
    sqlx::raw_sql("ROLLBACK")
        .execute(&mut *admin)
        .await
        .unwrap();
    match result {
        Err(error) => {
            assert_eq!(
                error
                    .as_database_error()
                    .and_then(|e| e.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
                    .map(|e| e.number()),
                Some(3572),
                "guard check failed for unrelated reason"
            );
            true
        }
        Ok(_) => false,
    }
}
async fn send_phase(socket: &mut tokio::net::TcpStream, id: &BranchIdentity, phase: BranchPhase) {
    let body = match phase {
        BranchPhase::Commit => Message::BranchCommitRequest(BranchCommitRequest {
            xid: id.xid().into(),
            branch_id: id.branch_id(),
            branch_type: BranchType::Xa,
            resource_id: id.resource().resource_id().into(),
            application_data: None,
        }),
        BranchPhase::Rollback => Message::BranchRollbackRequest(BranchRollbackRequest {
            xid: id.xid().into(),
            branch_id: id.branch_id(),
            branch_type: BranchType::Xa,
            resource_id: id.resource().resource_id().into(),
            application_data: None,
        }),
    };
    let mut request = peer::reply(-101, body);
    request.message_type = RequestType::RequestSync;
    peer::write(socket, request).await;
}

#[tokio::test]
#[ignore = "requires owned MySQL, owned forced process exits and real ACK interception"]
async fn managed_xa_process_crash_matrix() {
    let stages = [
        "enlisted",
        "active",
        "prepared_commit",
        "prepared_rollback",
        "commit_ok",
        "rollback_ok",
        "terminal_commit_ok",
        "terminal_rollback_ok",
        "unresponsive",
    ];
    let mut cases = Vec::new();
    for (index, stage) in stages.into_iter().enumerate() {
        let (mut admin, plain, _) = fixture::seed().await;
        let xid = format!("xa:process:{stage}");
        let branch = 9007199254741100 + index as i64;
        let phase = if matches!(
            stage,
            "prepared_commit" | "commit_ok" | "terminal_commit_ok"
        ) {
            BranchPhase::Commit
        } else {
            BranchPhase::Rollback
        };
        let native_id = seata_xa::XaBranchXid::new(Some(xid.clone()), branch);
        let intercepted = matches!(
            stage,
            "commit_ok" | "rollback_ok" | "terminal_commit_ok" | "terminal_rollback_ok"
        );
        let command = if stage.starts_with("terminal_") {
            b"\x03COMMIT".to_vec()
        } else {
            let mut bytes = vec![3];
            bytes.extend_from_slice(
                format!(
                    "XA {} {}",
                    if phase == BranchPhase::Commit {
                        "COMMIT"
                    } else {
                        "ROLLBACK"
                    },
                    native_id.mysql_arguments().unwrap()
                )
                .as_bytes(),
            );
            bytes
        };
        let proxy = if intercepted {
            Some(
                xa_command_proxy::XaCommandProxy::start(
                    std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
                    command.clone(),
                )
                .await,
            )
        } else {
            None
        };
        let port = proxy.as_ref().map(|p| p.port);
        let source = MySqlDataSource::connect(
            fixture::owned_mysql::options(port)
                .operation_timeout(if stage == "unresponsive" {
                    Duration::from_secs(1)
                } else {
                    Duration::from_secs(5)
                })
                .unwrap(),
        )
        .await
        .unwrap();
        let adapter = Arc::new(MySqlXaResourceManager::new(source.clone()).unwrap());
        let identity =
            BranchIdentity::new(xid.clone(), branch, adapter.key().clone(), None).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let folder = fixture::folder("processes").join(stage);
        std::fs::create_dir_all(folder.join("child")).unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "process_cases::managed_xa_crash_worker",
            "--ignored",
            "--exact",
            "--nocapture",
        ])
        .current_dir(&folder)
        .env("SEATA_XA_OUT", folder.join("child"))
        .env("SEATA_XA_PROCESS_STAGE", stage)
        .env(
            "SEATA_RM_ENDPOINT",
            listener.local_addr().unwrap().to_string(),
        )
        .stdout(File::create(folder.join("child.stdout")).unwrap())
        .stderr(File::create(folder.join("child.stderr")).unwrap());
        if let Some(port) = port {
            cmd.env("SEATA_MYSQL_PORT", port.to_string());
        }
        let mut child = xa_owned_process::XaOwnedProcess::spawn(cmd);
        let mut child_socket = tokio::time::timeout(
            Duration::from_secs(15),
            bootstrap(&listener, identity.resource().resource_id()),
        )
        .await
        .unwrap();
        registered(
            &mut child_socket,
            identity.resource().resource_id(),
            &xid,
            None,
            branch,
        )
        .await;
        let checkpoint = folder.join("child/proof/process-checkpoint.json");
        let observed: serde_json::Value = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Ok(bytes) = std::fs::read(&checkpoint) {
                    if let Ok(value) = serde_json::from_slice(&bytes) {
                        break value;
                    }
                }
                assert!(child.alive(), "writer exited before checkpoint");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            observed["rust_process_id"].as_u64(),
            Some(child.pid() as u64)
        );
        assert_ne!(child.pid(), std::process::id());
        assert_eq!(observed["xid"], xid);
        assert_eq!(observed["branch_id"], branch.to_string());
        assert_eq!(observed["resource_id"], identity.resource().resource_id());
        let prepared_before = seata_db_mysql::MySqlXaDataSource::new(source.clone())
            .prepared_observed(&native_id)
            .await
            .unwrap();
        assert_eq!(prepared_before, observed["prepared"].as_bool().unwrap());
        assert!(fixture::business(&mut admin).await.is_empty());
        assert!(
            guard_locked(&mut admin, &identity).await,
            "checkpoint did not hold actual XA guard"
        );
        let mut packet = None;
        if let Some(proxy) = proxy.as_ref() {
            if stage == "terminal_rollback_ok" {
                proxy.arm_after(2);
            } else if stage == "terminal_commit_ok" {
                proxy.arm_after(1);
            } else {
                proxy.arm();
            }
            send_phase(&mut child_socket, &identity, phase).await;
            packet = Some(
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        if let Some(value) = proxy.captured() {
                            break value;
                        }
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap(),
            );
            let row: (u8, u8) = sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_branch_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(
                row,
                (
                    if phase == BranchPhase::Commit { 1 } else { 2 },
                    u8::from(stage.starts_with("terminal_"))
                ),
                "intercepted wrong control boundary"
            );
        }
        let mut control_before_crash: (u8, u8) =
            sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_branch_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
        let prepared_at_cut = seata_db_mysql::MySqlXaDataSource::new(source.clone())
            .prepared_observed(&native_id)
            .await
            .unwrap();
        let business_at_cut = fixture::business(&mut admin).await;
        let mut exit_code = None;
        let mut first_unresponsive = None;
        let recovery_store = if stage == "unresponsive" {
            journal(&folder.join("parent.log")).await
        } else {
            let status = child.crash();
            assert!(!status.success());
            exit_code = status.code();
            journal(&folder.join("child/owner/branches.log")).await
        };
        let original_state = recovery_store
            .lease(identity.clone(), phase)
            .await
            .unwrap()
            .state();
        assert_eq!(
            original_state,
            if intercepted {
                Some(BranchRecordState::Pending)
            } else {
                None
            },
            "old durable RM journal differs from actual crash boundary"
        );
        if !intercepted && prepared_before && stage != "unresponsive" {
            assert!(
                seata_db_mysql::MySqlXaDataSource::new(source.clone())
                    .prepared_observed(&native_id)
                    .await
                    .unwrap(),
                "Prepared vanished after actual producer process death"
            );
        }
        drop(child_socket);
        let recovery_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let recovery_source = MySqlDataSource::connect(
            fixture::owned_mysql::options(port)
                .operation_timeout(if stage == "unresponsive" {
                    Duration::from_secs(1)
                } else {
                    Duration::from_secs(5)
                })
                .unwrap(),
        )
        .await
        .unwrap();
        let recovery_adapter =
            Arc::new(MySqlXaResourceManager::new(recovery_source.clone()).unwrap());
        let recovery = manager(
            recovery_adapter.clone(),
            recovery_listener.local_addr().unwrap(),
            recovery_store.clone(),
        )
        .await;
        let (start, ready) = tokio::sync::oneshot::channel();
        let (done, finished) = tokio::sync::oneshot::channel();
        let original = identity.clone();
        let key = adapter.key().clone();
        let slow = stage == "unresponsive";
        let (first, waiting) = tokio::sync::oneshot::channel();
        let (resume, continued) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let mut socket = tokio::time::timeout(
                Duration::from_secs(15),
                bootstrap(&recovery_listener, key.resource_id()),
            )
            .await
            .unwrap();
            ready.await.unwrap();
            if slow {
                assert_eq!(
                    callback(&mut socket, -201, &original, phase).await.1,
                    9,
                    "unresponsive live guard falsely completed"
                );
                first.send(()).unwrap();
                continued.await.unwrap();
            }
            let expected = if phase == BranchPhase::Commit { 5 } else { 8 };
            let mut statuses = Vec::new();
            for retry in 0..4 {
                let response = callback(&mut socket, -202 - retry, &original, phase).await;
                statuses.push(response.1);
                if response == (ResultCode::Success, expected) {
                    break;
                }
                assert_eq!(
                    response.1,
                    if phase == BranchPhase::Commit { 6 } else { 9 },
                    "process recovery became unretryable"
                );
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            assert_eq!(
                statuses.last(),
                Some(&expected),
                "process recovery did not finish original branch"
            );
            done.send(statuses).unwrap();
            idle(&mut socket).await;
        });
        start.send(()).unwrap();
        if stage == "unresponsive" {
            waiting.await.unwrap();
            assert!(child.alive());
            assert!(guard_locked(&mut admin, &identity).await);
            assert_eq!(
                recovery_store
                    .lease(identity.clone(), phase)
                    .await
                    .unwrap()
                    .state(),
                Some(BranchRecordState::Pending)
            );
            let completed: u8 = sqlx::query_scalar("SELECT completed FROM seata_rs_xa_branch_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
            assert_eq!(completed, 0);
            control_before_crash =
                sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_branch_v1")
                    .fetch_one(&mut admin)
                    .await
                    .unwrap();
            first_unresponsive = Some(9);
            exit_code = child.crash().code();
            resume.send(()).unwrap();
        }
        let statuses = finished.await.unwrap();
        assert_eq!(
            recovery_store
                .lease(identity.clone(), phase)
                .await
                .unwrap()
                .state(),
            Some(BranchRecordState::Completed)
        );
        let business = fixture::business(&mut admin).await;
        assert_eq!(
            business,
            if phase == BranchPhase::Commit {
                vec![(1, 42)]
            } else {
                vec![]
            }
        );
        assert!(!seata_db_mysql::MySqlXaDataSource::new(source.clone())
            .prepared_observed(&native_id)
            .await
            .unwrap());
        let control_after: (u8, u8) =
            sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_branch_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
        let guard_after: Option<(u8, u8)> =
            sqlx::query_as("SELECT phase,completed FROM seata_rs_xa_session_guard_v1")
                .fetch_optional(&mut admin)
                .await
                .unwrap();
        let witness_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_xa_commit_witness_v1")
                .fetch_one(&mut admin)
                .await
                .unwrap();
        recovery.shutdown().await.unwrap();
        server.await.unwrap();
        recovery_source.close().await.unwrap();
        source.close().await.unwrap();
        plain.close().await.unwrap();
        let matched = if let Some(proxy) = proxy {
            Some(proxy.shutdown().await)
        } else {
            None
        };
        cases.push(serde_json::json!({"stage":stage,"xid":xid,"branch_id":branch.to_string(),"writer_pid":child.pid(),"recovery_pid":std::process::id(),"writer_exited":!child.alive(),"writer_exit_code":exit_code,"checkpoint_pid":observed["rust_process_id"],"prepared_at_checkpoint":prepared_before,"prepared_at_cut":prepared_at_cut,"guard_locked_at_checkpoint":true,"control_before_crash":control_before_crash,"business_at_cut":business_at_cut,"control_after":control_after,"guard_after":guard_after,"witness_after":witness_after,"recovery_source_created_after_exit":stage!="unresponsive","recovery_statuses":statuses,"old_journal":if intercepted{"Pending"}else{"None"},"held_packet":packet,"matched_commands":matched,"first_unresponsive_status":first_unresponsive,"journal":"Completed","business":business,"prepared_remaining":false,"callback_status":if phase==BranchPhase::Commit{5}else{8}}));
    }
    fixture::record(
        "process-crash-matrix",
        serde_json::json!({"cases":cases,"whole_plan_complete":false}),
    );
}
