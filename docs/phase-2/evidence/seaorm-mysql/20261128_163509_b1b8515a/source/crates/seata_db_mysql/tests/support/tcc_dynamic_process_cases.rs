//! 真实 TCC 写入子进程强制终止、同库重启及新 RM 的原日志恢复。
#![allow(clippy::unwrap_used)]
use super::{command_proxy::XaCommandProxy, fixture, manager, peer};
#[path = "tcc_dynamic_crash_action.rs"]
mod dynamic_crash_action;
use super::process_cases::{owned_process, tcc_crash_action};
use seata_db_mysql::{MySqlDataSource, MySqlTccResourceManager};
use seata_protocol::{transaction::BranchRegisterRequest, BranchType, ResultCode};
use seata_rm::{BranchIdentity, BranchPhase, BranchRecordState, BranchResource};
use std::{fs::File, path::Path, process::Command, sync::Arc, time::Duration};
use tokio::net::TcpListener;

async fn json_file(path: &Path) -> serde_json::Value {
    tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(value) = serde_json::from_slice(&bytes) {
                    break value;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "only invoked as an owned TCC writer process"]
async fn dynamic_tcc_crash_worker() {
    let stage = std::env::var("SEATA_TCC_PROCESS_STAGE").unwrap();
    // 即使父夹具被终止，本自有写入者也只在自己的进程内于期限后退出。
    let _watchdog = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(100));
        std::process::exit(91);
    });
    let source = MySqlDataSource::connect(
        fixture::owned_mysql::options(None)
            .operation_timeout(Duration::from_secs(60))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = Arc::new(
        MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            Arc::new(dynamic_crash_action::TccDynamicCrashAction {
                stage: stage.clone(),
            }),
        )
        .unwrap()
        .with_context_reporting(),
    );
    let listener_endpoint = std::env::var("SEATA_RM_ENDPOINT").unwrap().parse().unwrap();
    let catalog = seata_rm::ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let store = seata_rm::FileBranchStore::open(
        fixture::folder("owner").join("branches.log"),
        seata_rm::JournalOptions::new(32, 1048576, 65536, 32).unwrap(),
    )
    .await
    .unwrap();
    let rm = seata_rm::ResourceManager::builder(
        fixture::pool(listener_endpoint),
        fixture::registration(false),
        catalog,
        store,
    )
    .request_timeout(Duration::from_secs(60))
    .callback_timeout(Duration::from_secs(60))
    .build()
    .unwrap();
    let xid = format!("tcc:dynamic-process:{stage}");
    let receipt = rm
        .client()
        .branch_register_with_receipt(BranchRegisterRequest {
            xid: Some(xid),
            branch_type: BranchType::Tcc,
            resource_id: Some(adapter.key().resource_id().into()),
            lock_key: None,
            application_data: Some(fixture::app(81)),
        })
        .await
        .unwrap();
    fixture::record(
        "receipt",
        serde_json::json!({"stage":stage,"xid":receipt.identity().xid(),"branch_id":receipt.identity().branch_id().to_string(),"resource_id":receipt.identity().resource().resource_id(),"application_data":receipt.identity().application_data()}),
    );
    if stage != "suspended_ok" {
        adapter
            .try_registered_with_context(&rm.client(), receipt.clone())
            .await
            .unwrap();
        fixture::record("tried", serde_json::json!({"stage":stage}));
    }
    let owned = (receipt, rm, source, adapter);
    std::future::pending::<()>().await;
    drop(owned);
}

async fn row(admin: &mut sqlx::MySqlConnection) -> Option<u8> {
    sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
        .fetch_optional(admin)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires owned MySQL, forced writer exit and optional database restart"]
async fn dynamic_tcc_process_crash_matrix() {
    let mode = std::env::var("SEATA_TCC_DATABASE_FAULT").unwrap();
    assert!(["none", "restart", "crash"].contains(&mode.as_str()));
    let stages = [
        "try_uncommitted",
        "report_waiting",
        "tried_commit",
        "tried_cancel",
        "try_ok",
        "confirm_uncommitted",
        "cancel_uncommitted",
        "confirm_ok",
        "cancel_ok",
        "suspended_ok",
    ];
    let mut cases = Vec::new();
    for (index, stage) in stages.into_iter().enumerate() {
        let (mut admin, seed_source, adapter) = fixture::seed().await;
        seed_source.close().await.unwrap();
        sqlx::raw_sql("DROP TABLE IF EXISTS seata_rs_tcc_context_v1")
            .execute(&mut admin)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/020_seata_rs_tcc_context_v1.sql"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        let uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        let xid = format!("tcc:dynamic-process:{stage}");
        let branch = 9400 + index as i64;
        let phase = if matches!(
            stage,
            "tried_commit" | "try_ok" | "confirm_uncommitted" | "confirm_ok"
        ) {
            BranchPhase::Commit
        } else {
            BranchPhase::Rollback
        };
        let identity = BranchIdentity::new(
            xid.clone(),
            branch,
            adapter.key().clone(),
            Some(fixture::app(81)),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let folder = fixture::folder("processes").join(stage);
        std::fs::create_dir_all(folder.join("child")).unwrap();
        let proxy = if stage.ends_with("_ok") {
            Some(
                XaCommandProxy::start(
                    std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
                    b"\x03COMMIT".to_vec(),
                )
                .await,
            )
        } else {
            None
        };
        if stage == "try_ok" {
            proxy.as_ref().unwrap().arm_after(2);
        }
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "dynamic_process_cases::dynamic_tcc_crash_worker",
                "--ignored",
                "--exact",
                "--nocapture",
            ])
            .current_dir(&folder)
            .env("SEATA_XA_OUT", folder.join("child"))
            .env("SEATA_TCC_PROCESS_STAGE", stage)
            .env(
                "SEATA_RM_ENDPOINT",
                listener.local_addr().unwrap().to_string(),
            )
            .stdout(File::create(folder.join("child.stdout")).unwrap())
            .stderr(File::create(folder.join("child.stderr")).unwrap());
        if let Some(proxy) = &proxy {
            command.env("SEATA_MYSQL_PORT", proxy.port.to_string());
        }
        let mut child = owned_process::XaOwnedProcess::spawn(command);
        let mut socket = tokio::time::timeout(
            Duration::from_secs(15),
            peer::bootstrap(&listener, adapter.key().resource_id()),
        )
        .await
        .unwrap();
        peer::registered(
            &mut socket,
            adapter.key().resource_id(),
            &xid,
            identity.application_data(),
            branch,
        )
        .await;
        let callback_identity = if stage != "suspended_ok" {
            let (request_id, app) = peer::read_context_report(&mut socket, &identity).await;
            let parsed: serde_json::Value = serde_json::from_str(&app).unwrap();
            assert_eq!(parsed["actionContext"]["amount"], 9);
            assert_eq!(parsed["actionContext"]["id"], 81);
            if stage != "report_waiting" {
                peer::reply_context_report(&mut socket, request_id, ResultCode::Success).await;
            }
            BranchIdentity::new(xid.clone(), branch, adapter.key().clone(), Some(app)).unwrap()
        } else {
            identity.clone()
        };
        let observed = json_file(&folder.join("child/proof/receipt.json")).await;
        assert_eq!(
            observed["rust_process_id"].as_u64(),
            Some(child.pid() as u64)
        );
        assert_ne!(child.pid(), std::process::id());
        assert_eq!(observed["xid"], xid);
        assert_eq!(observed["branch_id"], branch.to_string());
        assert_eq!(observed["application_data"], fixture::app(81));
        if !matches!(
            stage,
            "try_uncommitted" | "report_waiting" | "try_ok" | "suspended_ok"
        ) {
            json_file(&folder.join("child/proof/tried.json")).await;
        }
        let terminal = stage.starts_with("confirm_")
            || stage.starts_with("cancel_")
            || stage == "suspended_ok";
        let mut held_report_socket = None;
        let callback = if terminal {
            if let Some(proxy) = &proxy {
                proxy.arm_after(usize::from(stage == "cancel_ok"));
            }
            let original = callback_identity.clone();
            Some(tokio::spawn(async move {
                peer::callback(&mut socket, -301, &original, phase).await
            }))
        } else if stage == "report_waiting" {
            held_report_socket = Some(socket);
            None
        } else {
            drop(socket);
            None
        };
        let mut packet = None;
        if let Some(proxy) = &proxy {
            packet = Some(
                tokio::time::timeout(Duration::from_secs(15), async {
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
            let (request, ok) = packet.as_ref().unwrap();
            assert_eq!(&request[4..], b"\x03COMMIT");
            assert_eq!(ok[4], 0);
        }
        let mut locked = false;
        if stage.ends_with("uncommitted") || stage == "report_waiting" {
            let checkpoint = json_file(&folder.join("child/proof/uncommitted.json")).await;
            assert_eq!(
                checkpoint["rust_process_id"].as_u64(),
                Some(child.pid() as u64)
            );
            sqlx::raw_sql("START TRANSACTION")
                .execute(&mut admin)
                .await
                .unwrap();
            let error=sqlx::query("SELECT status FROM seata_rs_tcc_fence_v1 WHERE xid=? AND branch_id=? FOR UPDATE NOWAIT")
                .bind(xid.as_bytes()).bind(branch).fetch_optional(&mut admin).await.unwrap_err();
            assert_eq!(
                error
                    .as_database_error()
                    .and_then(|e| e.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
                    .map(|e| e.number()),
                Some(3572)
            );
            sqlx::raw_sql("ROLLBACK").execute(&mut admin).await.unwrap();
            locked = true;
        }
        let at_cut = row(&mut admin).await;
        let business_at_cut = fixture::business(&mut admin).await;
        let expected_cut = match stage {
            "try_uncommitted" | "report_waiting" => None,
            "confirm_ok" => Some(2),
            "cancel_ok" => Some(3),
            "suspended_ok" => Some(4),
            _ => Some(1),
        };
        assert_eq!(at_cut, expected_cut);
        assert_eq!(
            business_at_cut,
            match at_cut {
                Some(1) => vec![(81, 7)],
                Some(2) => vec![(81, 18)],
                _ => vec![],
            }
        );
        std::fs::write(
            folder.join("cut-request.json"),
            serde_json::to_vec(
                &serde_json::json!({"stage":stage,"writer_pid":child.pid(),"uuid":uuid}),
            )
            .unwrap(),
        )
        .unwrap();
        let cut = json_file(&folder.join("cut-ready.json")).await;
        assert_eq!(cut["writer_alive"], true);
        assert!(child.alive());
        let exit = child.crash();
        drop(held_report_socket);
        assert!(!child.alive());
        if let Some(callback) = callback {
            callback.abort();
            let _ = callback.await;
        }
        let matched = if let Some(proxy) = proxy {
            Some(proxy.shutdown().await)
        } else {
            None
        };
        drop(admin);
        // 数据库生命周期由外层拥有容器句柄的运行器控制；不由库代码操作 Docker。
        {
            std::fs::write(folder.join("restart-request.json"),serde_json::to_vec(&serde_json::json!({"stage":stage,"mode":mode,"writer_exited":true,"uuid":uuid})).unwrap()).unwrap();
            let ready = json_file(&folder.join("restart-ready.json")).await;
            assert_eq!(ready["mode"], mode);
            assert_eq!(ready["uuid"], uuid);
        }
        let mut admin = fixture::owned_mysql::admin().await;
        let after_uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(after_uuid, uuid);
        let status_after_exit = row(&mut admin).await;
        assert_eq!(status_after_exit, expected_cut);
        assert_eq!(fixture::business(&mut admin).await, business_at_cut);
        let fresh_source = MySqlDataSource::connect(fixture::owned_mysql::options(None))
            .await
            .unwrap();
        let fresh = Arc::new(
            MySqlTccResourceManager::new(
                fresh_source.clone(),
                "tcc-original-action".into(),
                Arc::new(fixture::Action::default()),
            )
            .unwrap()
            .with_context_reporting(),
        );
        let recovery_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let old = folder.join("child/owner/branches.log");
        // manager 使用同一文件路径，恢复旧 Pending，不能新造成功日志。
        let name = old.parent().unwrap().to_string_lossy().into_owned();
        let (rm, store) = manager(fresh.clone(), &recovery_listener, &name).await;
        let old_state = store.lease(identity.clone(), phase).await.unwrap().state();
        assert_eq!(
            old_state,
            if terminal {
                Some(BranchRecordState::Pending)
            } else {
                None
            }
        );
        let original = callback_identity.clone();
        let server = tokio::spawn(async move {
            let mut socket =
                peer::bootstrap(&recovery_listener, original.resource().resource_id()).await;
            let mut attempts = Vec::new();
            for id in [-302, -303] {
                for retry in 0..8 {
                    let request_id = id - retry * 100;
                    let response = peer::callback_with_read_timeout(
                        &mut socket,
                        request_id,
                        &original,
                        phase,
                        Duration::from_secs(7),
                    )
                    .await;
                    attempts.push(serde_json::json!({"request_id":request_id,"result":format!("{:?}",response.0),"status":response.1}));
                    if response
                        == (
                            ResultCode::Success,
                            if phase == BranchPhase::Commit { 5 } else { 8 },
                        )
                    {
                        break;
                    }
                    assert_eq!(
                        response,
                        (
                            ResultCode::Failed,
                            if phase == BranchPhase::Commit { 6 } else { 9 }
                        )
                    );
                    assert!(retry < 7, "same-branch recovery retries exhausted");
                }
            }
            attempts
        });
        let recovery_callbacks = tokio::time::timeout(Duration::from_secs(90), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            store.lease(identity.clone(), phase).await.unwrap().state(),
            Some(BranchRecordState::Completed)
        );
        let final_status = row(&mut admin).await.unwrap();
        let expected = if matches!(stage, "try_uncommitted" | "report_waiting" | "suspended_ok") {
            4
        } else if phase == BranchPhase::Commit {
            2
        } else {
            3
        };
        assert_eq!(final_status, expected);
        let business = fixture::business(&mut admin).await;
        assert_eq!(
            business,
            if phase == BranchPhase::Commit {
                vec![(81, 18)]
            } else {
                vec![]
            }
        );
        rm.shutdown().await.unwrap();
        fresh_source.close().await.unwrap();
        std::fs::write(
            folder.join("finished-request.json"),
            serde_json::to_vec(&serde_json::json!({"stage":stage,"final_status":final_status}))
                .unwrap(),
        )
        .unwrap();
        json_file(&folder.join("finished-ready.json")).await;
        cases.push(serde_json::json!({"stage":stage,"database_fault":mode,"xid":xid,"branch_id":branch.to_string(),"writer_pid":child.pid(),"checkpoint_pid":observed["rust_process_id"],"recovery_pid":std::process::id(),"writer_exited":!child.alive(),"writer_exit_code":exit.code(),"uuid_before":uuid,"uuid_after":after_uuid,"actual_uncommitted_lock":locked,"fence_at_cut":at_cut,"business_at_cut":business_at_cut,"fence_after_exit":status_after_exit,"held_packet":packet,"matched_commands":matched,"old_journal":if terminal{"Pending"}else{"None"},"journal":"Completed","final_status":final_status,"business":business,"callback_status":if phase==BranchPhase::Commit{5}else{8},"reported_application_data":callback_identity.application_data(),"protocol":2,"recovery_callbacks":recovery_callbacks,"report_reply_confirmed":if stage=="suspended_ok"{None}else{Some(stage!="report_waiting")}}));
    }
    fixture::record(
        "tcc-dynamic-process-crash-matrix",
        serde_json::json!({"database_fault":mode,"cases":cases,"whole_plan_complete":false}),
    );
}
