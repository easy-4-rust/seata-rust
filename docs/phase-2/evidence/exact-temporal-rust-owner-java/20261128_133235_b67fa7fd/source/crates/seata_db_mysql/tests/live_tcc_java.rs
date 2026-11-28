//! 固定原 Java TC 的正式 TCC 全局事务与真实同库 Fence/业务。
#![cfg(feature = "tcc")]
#![allow(clippy::unwrap_used)]
#[path = "support/xa_command_proxy.rs"]
mod command_proxy;
#[path = "support/tcc_dynamic_action.rs"]
mod dynamic_action;
#[path = "support/tcc_managed_fixture.rs"]
mod fixture;
#[path = "support/tcc_joint_action.rs"]
mod joint_action;
use seata_protocol::{transaction::BranchRegisterRequest, BranchType, GlobalStatus};
use seata_rm::{
    BranchPhase, BranchRecordState, BranchResource, FileBranchStore, JournalOptions,
    ResourceCatalog, ResourceManager,
};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use std::time::Duration;

async fn run(rollback: bool) {
    let fault = std::env::var("SEATA_TCC_JAVA_FAULT").ok();
    if let Some(mode) = &fault {
        assert!(matches!(
            mode.as_str(),
            "duplicate" | "reconnect" | "report_lost" | "report_cancel" | "joint_crash"
        ));
    }
    let dynamic = std::env::var("SEATA_TCC_DYNAMIC").ok();
    let joint_cut = std::env::var("SEATA_TCC_JOINT_CUT").ok();
    if let Some(cut) = &joint_cut {
        assert!(["terminal_uncommitted", "terminal_ok"].contains(&cut.as_str()));
    }
    let mut mysql_proxy = if joint_cut.as_deref() == Some("terminal_ok") {
        Some(
            command_proxy::XaCommandProxy::start(
                std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
                b"\x03COMMIT".to_vec(),
            )
            .await,
        )
    } else {
        None
    };
    let report_fault = matches!(fault.as_deref(), Some("report_lost" | "report_cancel"));
    if report_fault {
        assert!(dynamic.is_some() && rollback);
    }
    let (mut admin, mut source, mut adapter) = fixture::seed().await;
    if let Some(mode) = &dynamic {
        assert!(["delayed", "immediate", "precision"].contains(&mode.as_str()));
        source.close().await.unwrap();
        source = seata_db_mysql::MySqlDataSource::connect(
            fixture::owned_mysql::options(mysql_proxy.as_ref().map(|p| p.port))
                .operation_timeout(Duration::from_secs(if joint_cut.is_some() {
                    60
                } else {
                    15
                }))
                .unwrap(),
        )
        .await
        .unwrap();
        sqlx::raw_sql("DROP TABLE IF EXISTS seata_rs_tcc_context_v1")
            .execute(&mut admin)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/020_seata_rs_tcc_context_v1.sql"
        ))
        .execute(&mut admin)
        .await
        .unwrap();
        let dynamic = dynamic_action::TccDynamicAction {
            immediate: mode == "immediate",
            precision_only: mode == "precision",
            catch_report_error: false,
            failure_mode: None,
        };
        let action: std::sync::Arc<dyn seata_tcc::TccBusinessAction> = if let Some(cut) = &joint_cut
        {
            std::sync::Arc::new(joint_action::TccJointAction {
                dynamic,
                cut: cut.clone(),
            })
        } else {
            std::sync::Arc::new(dynamic)
        };
        adapter = std::sync::Arc::new(
            seata_db_mysql::MySqlTccResourceManager::new(
                source.clone(),
                "tcc-original-action".into(),
                action,
            )
            .unwrap()
            .with_context_reporting(),
        );
    }
    let store = FileBranchStore::open(
        fixture::folder("java-journal").join("branches.log"),
        JournalOptions::new(32, 1048576, 65536, 32).unwrap(),
    )
    .await
    .unwrap();
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        fixture::pool(std::env::var("SEATA_RM_ENDPOINT").unwrap().parse().unwrap()),
        fixture::registration(true),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(if report_fault { 15 } else { 5 }))
    .callback_timeout(Duration::from_secs(if joint_cut.is_some() {
        60
    } else {
        5
    }))
    .build()
    .unwrap();
    let tm = TransactionManager::builder(
        fixture::pool(std::env::var("SEATA_TM_ENDPOINT").unwrap().parse().unwrap()),
        fixture::registration(true),
    )
    .request_timeout(Duration::from_secs(15))
    .build()
    .unwrap();
    let config = TransactionConfig::builder("native-managed-tcc".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut global = tm.begin(&config).await.unwrap();
    let app = if dynamic.as_deref() == Some("precision") {
        r#"{"actionContext":{"useTCCFence":true,"id":1,"amount":7,"precise":1.23000000000000000000000000001}}"#.into()
    } else {
        fixture::app(1)
    };
    let mut report_error_unknown = false;
    let mut try_future_cancelled = false;
    let receipt = if report_fault {
        // 先保留正式原回执，取消 Future 没有返回值，不能事后用裸字段重造回执。
        let receipt = rm
            .client()
            .branch_register_with_receipt(BranchRegisterRequest {
                xid: Some(global.xid().as_str().into()),
                branch_type: BranchType::Tcc,
                resource_id: Some(adapter.key().resource_id().into()),
                lock_key: None,
                application_data: Some(app.clone()),
            })
            .await
            .unwrap();
        if fault.as_deref() == Some("report_cancel") {
            let resource = adapter.clone();
            let client = rm.client();
            let retained = receipt.clone();
            let task = tokio::spawn(async move {
                resource
                    .try_registered_with_context(&client, retained)
                    .await
            });
            let limit = tokio::time::Instant::now() + Duration::from_secs(20);
            while !fixture::folder("control").join("cancel-try").exists() {
                assert!(
                    !task.is_finished(),
                    "Try completed before actual report cancellation boundary"
                );
                assert!(
                    tokio::time::Instant::now() < limit,
                    "owned report cancellation marker missing"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(!task.is_finished());
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            try_future_cancelled = true;
        } else {
            let error = adapter
                .try_registered_with_context(&rm.client(), receipt.clone())
                .await
                .unwrap_err();
            assert_eq!(
                error.registered_branch().unwrap().identity(),
                receipt.identity()
            );
            let seata_db_mysql::MySqlTccTryError::ContextReport {
                source: reason,
                context_report_started,
                ..
            } = error
            else {
                panic!("original report error type lost")
            };
            assert!(context_report_started);
            let seata_db_mysql::MySqlTccContextReportError::Rpc(reason) = *reason else {
                panic!("report loss became a local error")
            };
            assert!(reason.is_unknown_outcome());
            report_error_unknown = true;
        }
        assert!(fixture::business(&mut admin).await.is_empty());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
        assert_eq!(count, 0, "unconfirmed report allowed Try commit");
        receipt
    } else {
        adapter
            .register_and_try(&rm.client(), global.xid().as_str(), Some(app.clone()))
            .await
            .unwrap()
    };
    let identity = receipt.identity().clone();
    assert_eq!(receipt.lock_key(), None);
    assert_eq!(
        fixture::business(&mut admin).await,
        if report_fault { vec![] } else { vec![(1, 7)] }
    );
    let mut reported_data = None;
    if let Some(mode) = &dynamic {
        assert_eq!(identity.application_data(), Some(app.as_str()));
        let current: Vec<u8> = sqlx::query_scalar(
            "SELECT application_data FROM seata_rs_tcc_context_v1 ORDER BY revision DESC LIMIT 1",
        )
        .fetch_one(&mut admin)
        .await
        .unwrap();
        let current = String::from_utf8(current).unwrap();
        let tc_data: String =
            sqlx::query_scalar("SELECT application_data FROM branch_table WHERE branch_id=?")
                .bind(identity.branch_id())
                .fetch_one(&mut admin)
                .await
                .unwrap();
        assert_eq!(tc_data, current);
        fixture::record(
            "dynamic-before-end",
            serde_json::json!({"mode":mode,"fault":fault,"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"registered_application_data":app,"reported_application_data":current,"try_committed":!report_fault,"try_future_cancelled":try_future_cancelled,"report_error_unknown":report_error_unknown}),
        );
        reported_data = Some(current);
        if let Some(proxy) = mysql_proxy.take() {
            if rollback {
                proxy.arm_after(1);
            } else {
                proxy.arm();
            }
            let cut_identity = identity.clone();
            let _mysql_watcher = tokio::spawn(async move {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
                loop {
                    if let Some((request, ok)) = proxy.captured() {
                        fixture::record(
                            "joint-terminal-cut",
                            serde_json::json!({"cut":"terminal_ok","phase":if rollback{"cancel"}else{"confirm"},"xid":cut_identity.xid(),"branch_id":cut_identity.branch_id().to_string(),"held_request":request,"held_ok":ok,"matched_commands":proxy.matched_commands()}),
                        );
                        std::future::pending::<()>().await;
                        drop(proxy);
                        return;
                    }
                    if tokio::time::Instant::now() >= deadline {
                        proxy.shutdown().await;
                        panic!("actual terminal COMMIT OK capture deadline");
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            });
        }
        if std::env::var_os("SEATA_TCC_DYNAMIC_HOLD").is_some() {
            let limit = tokio::time::Instant::now() + Duration::from_secs(20);
            while !fixture::folder("control").join("allow-end").exists() {
                assert!(
                    tokio::time::Instant::now() < limit,
                    "independent TC report observation missing"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
    let result = if rollback {
        global.rollback().await
    } else {
        global.commit().await
    };
    let observed = match result {
        Ok(TransactionCompletion::Observed { outcome, .. }) => Some(outcome.status()),
        other if fault.is_some() => {
            let _ = other;
            None
        }
        other => panic!("original TCC global end unresolved: {other:?}"),
    };
    let expected = if rollback {
        GlobalStatus::Rollbacked
    } else {
        GlobalStatus::Committed
    };
    if fault.is_none() {
        assert_eq!(observed, Some(expected));
    }
    // 只等待原 TC 的原事务完成；未决客户端结果保持未决，不用 Finished 猜方向。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(25);
    loop {
        let remaining:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM global_table)+(SELECT COUNT(*) FROM branch_table)+(SELECT COUNT(*) FROM lock_table)").fetch_one(&mut admin).await.unwrap();
        if remaining == 0 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC did not complete original TCC after retry"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let phase = if rollback {
        BranchPhase::Rollback
    } else {
        BranchPhase::Commit
    };
    assert_eq!(
        store.lease(identity.clone(), phase).await.unwrap().state(),
        Some(BranchRecordState::Completed)
    );
    let business = fixture::business(&mut admin).await;
    assert_eq!(
        business,
        if rollback {
            vec![]
        } else {
            vec![(
                1,
                if dynamic.is_some() && dynamic.as_deref() != Some("precision") {
                    18
                } else {
                    14
                },
            )]
        }
    );
    let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        status,
        if report_fault {
            4
        } else if rollback {
            3
        } else {
            2
        }
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM global_table)+(SELECT COUNT(*) FROM branch_table)+(SELECT COUNT(*) FROM lock_table)").fetch_one(&mut admin).await.unwrap();
        if remaining == 0 {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    if let Some(mode) = dynamic {
        if let Some(fault_mode) = fault {
            fixture::record(
                "original-java-tcc-dynamic-fault",
                serde_json::json!({"mode":mode,"fault":fault_mode,"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"resource_id":adapter.key().resource_id(),"branch_type":identity.resource().branch_type().to_java_v1(),"registered_application_data":app,"reported_application_data":reported_data,"lock_key":null,"initial_global_status":observed.map(|v|format!("{v:?}")),"expected_tc_terminal":format!("{expected:?}"),"status":status,"journal":"Completed","business":business,"try_committed":!report_fault,"try_future_cancelled":try_future_cancelled,"report_error_unknown":report_error_unknown,"original_receipt_retained":true,"client_end_requests":1,"finished_inferred_as_success":false}),
            );
        } else {
            fixture::record(
                "original-java-tcc-dynamic",
                serde_json::json!({"mode":mode,"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"resource_id":adapter.key().resource_id(),"branch_type":identity.resource().branch_type().to_java_v1(),"registered_application_data":app,"reported_application_data":reported_data,"global_status":format!("{:?}",observed.unwrap()),"status":status,"journal":"Completed","business":business,"finished_inferred_as_success":false}),
            );
        }
    } else if let Some(mode) = fault {
        fixture::record(
            "original-java-tcc-fault",
            serde_json::json!({"mode":mode,"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"resource_id":adapter.key().resource_id(),"branch_type":identity.resource().branch_type().to_java_v1(),"application_data":app,"lock_key":null,"initial_global_status":observed.map(|v|format!("{v:?}")),"expected_tc_terminal":format!("{expected:?}"),"status":status,"journal":"Completed","business":business,"client_end_requests":1,"finished_inferred_as_success":false}),
        );
    } else {
        fixture::record(
            "original-java-tcc",
            serde_json::json!({"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"resource_id":adapter.key().resource_id(),"branch_type":identity.resource().branch_type().to_java_v1(),"application_data":app,"lock_key":null,"global_status":format!("{:?}",observed.unwrap()),"status":status,"journal":"Completed","business":business,"finished_inferred_as_success":false}),
        );
    }
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires pinned original Java TC and owned MySQL"]
async fn original_java_tc_commits_original_tcc_branch() {
    run(false).await;
}
#[tokio::test]
#[ignore = "requires pinned original Java TC and owned MySQL"]
async fn original_java_tc_rolls_back_original_tcc_branch() {
    run(true).await;
}

/// 新进程只重开原日志并接收原 TC 回调；不重新取得或重造登记回执。
#[tokio::test]
#[ignore = "requires original Java TC joint-crash controller and owned database"]
async fn original_java_tc_recovers_joint_crash_tcc_branch() {
    let before: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("SEATA_TCC_JOINT_BEFORE").unwrap()).unwrap(),
    )
    .unwrap();
    let mode = before["mode"].as_str().unwrap();
    assert!(["delayed", "immediate", "precision"].contains(&mode));
    let direction = std::env::var("SEATA_TCC_JOINT_DIRECTION").unwrap();
    assert!(["commit", "rollback"].contains(&direction.as_str()));
    let phase = if direction == "commit" {
        BranchPhase::Commit
    } else {
        BranchPhase::Rollback
    };
    let source = seata_db_mysql::MySqlDataSource::connect(
        fixture::owned_mysql::options(None)
            .operation_timeout(Duration::from_secs(15))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = std::sync::Arc::new(
        seata_db_mysql::MySqlTccResourceManager::new(
            source.clone(),
            "tcc-original-action".into(),
            std::sync::Arc::new(fixture::Action::default()),
        )
        .unwrap()
        .with_context_reporting(),
    );
    let identity = seata_rm::BranchIdentity::new(
        before["xid"].as_str().unwrap().into(),
        before["branch_id"].as_str().unwrap().parse().unwrap(),
        adapter.key().clone(),
        Some(
            before["registered_application_data"]
                .as_str()
                .unwrap()
                .into(),
        ),
    )
    .unwrap();
    let journal = std::path::PathBuf::from(std::env::var("SEATA_TCC_OLD_JOURNAL").unwrap());
    let cut = std::env::var("SEATA_TCC_JOINT_CUT").ok();
    if cut.is_none() {
        assert_eq!(std::fs::read(&journal).unwrap(), b"SRMJNL01");
    }
    let store = FileBranchStore::open(
        journal,
        JournalOptions::new(32, 1048576, 65536, 32).unwrap(),
    )
    .await
    .unwrap();
    if cut.is_some() {
        assert_eq!(
            store.lease(identity.clone(), phase).await.unwrap().state(),
            Some(BranchRecordState::Pending)
        );
    }
    let catalog = ResourceCatalog::new(4, 32767).unwrap();
    catalog.register(adapter.clone()).unwrap();
    let rm = ResourceManager::builder(
        fixture::pool(std::env::var("SEATA_RM_ENDPOINT").unwrap().parse().unwrap()),
        fixture::registration(true),
        catalog,
        store.clone(),
    )
    .request_timeout(Duration::from_secs(5))
    .callback_timeout(Duration::from_secs(15))
    .build()
    .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    loop {
        let state = store
            .lease(identity.clone(), phase)
            .await
            .ok()
            .and_then(|lease| lease.state());
        if state == Some(BranchRecordState::Completed) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC did not recover durable decision"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let mut admin = fixture::owned_mysql::admin().await;
    let row: (u8, u16, Vec<u8>) =
        sqlx::query_as("SELECT status,schema_version,application_data FROM seata_rs_tcc_fence_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    let status = if direction == "commit" { 2 } else { 3 };
    assert_eq!((row.0, row.1), (status, 2));
    assert_eq!(row.2, identity.application_data().unwrap().as_bytes());
    let business = fixture::business(&mut admin).await;
    assert_eq!(
        business,
        if direction == "rollback" {
            vec![]
        } else {
            vec![(1, if mode == "precision" { 14 } else { 18 })]
        }
    );
    loop {
        let remaining:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM global_table)+(SELECT COUNT(*) FROM branch_table)+(SELECT COUNT(*) FROM lock_table)").fetch_one(&mut admin).await.unwrap();
        if remaining == 0 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC terminal cleanup not observed"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    fixture::record(
        "original-java-tcc-joint-recovery",
        serde_json::json!({"mode":mode,"direction":direction,"cut":cut,"old_journal":if cut.is_some(){"Pending"}else{"None"},"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"registered_application_data":identity.application_data(),"reported_application_data":before["reported_application_data"],"writer_pid":before["rust_process_id"],"journal":"Completed","status":status,"protocol":2,"business":business,"new_global_end_requests":0,"new_business_registrations":0,"new_reports":0,"finished_inferred_as_success":false}),
    );
    rm.shutdown().await.unwrap();
    source.close().await.unwrap();
}
