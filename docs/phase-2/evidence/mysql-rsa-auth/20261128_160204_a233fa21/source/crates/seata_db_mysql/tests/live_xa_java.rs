//! 固定原 Java TC 的正常 XA 全局 Commit/Rollback，使用正式登记、持久控制和真实 MySQL。
#![cfg(feature = "xa")]
#![allow(clippy::unwrap_used)]
#[path = "support/xa_managed_fixture.rs"]
mod fixture;
use seata_at::SqlRecognizerFactory;
use seata_datasource::FieldValue;
use seata_protocol::GlobalStatus;
use seata_rm::{
    BranchPhase, BranchRecordState, BranchResource, FileBranchStore, JournalOptions,
    ResourceCatalog, ResourceManager,
};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use std::time::Duration;

async fn run(rollback: bool) {
    let fault = std::env::var("SEATA_XA_JAVA_FAULT").ok();
    let database_restart = matches!(fault.as_deref(), Some("restart" | "crash"));
    let (mut admin, source, adapter) = fixture::seed().await;
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
    .request_timeout(Duration::from_secs(3))
    .callback_timeout(Duration::from_secs(5))
    .build()
    .unwrap();
    let tm = TransactionManager::builder(
        fixture::pool(std::env::var("SEATA_TM_ENDPOINT").unwrap().parse().unwrap()),
        fixture::registration(true),
    )
    .request_timeout(Duration::from_secs(15))
    .build()
    .unwrap();
    let config = TransactionConfig::builder("native-managed-xa".into())
        .transaction_timeout(Duration::from_secs(if database_restart { 180 } else { 60 }))
        .build()
        .unwrap();
    let mut global = tm.begin(&config).await.unwrap();
    let xid = global.xid().clone();
    let mut local = adapter
        .register_and_start(&rm.client(), xid.as_str(), None)
        .await
        .unwrap();
    let receipt = local.registered_branch().clone();
    let identity = receipt.identity().clone();
    assert_eq!(
        identity.resource().branch_type(),
        seata_protocol::BranchType::Xa
    );
    assert_eq!(receipt.lock_key(), None);
    local
        .execute(
            &SqlRecognizerFactory::recognize("INSERT INTO xa_managed(id,v) VALUES(?,?)").unwrap(),
            &[FieldValue::Signed(1), FieldValue::Signed(42)],
        )
        .await
        .unwrap();
    let prepared = local.prepare().await.unwrap();
    assert_eq!(prepared.identity(), &identity);
    assert!(fixture::business(&mut admin).await.is_empty());
    if database_restart {
        // 只暂停已真实 Prepare 的原事务；外部运行器核对自有容器后重启同一实例。
        fixture::record(
            "before-database-restart",
            serde_json::json!({"mode":fault,"xid":xid.as_str(),"branch_id":identity.branch_id().to_string(),"resource_id":adapter.key().resource_id()}),
        );
        std::fs::write(
            fixture::folder("proof").join("database-checkpoint-ready"),
            b"prepared\n",
        )
        .unwrap();
        let resumed = fixture::folder("proof").join("database-resumed");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        while !resumed.exists() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "owned database restart not resumed"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert_eq!(std::fs::read(resumed).unwrap(), b"resume-original-branch\n");
        // 原观察连接已随数据库退出而关闭，恢复后使用新的独立观察会话。
        admin = fixture::owned_mysql::admin().await;
    }
    let phase = if rollback {
        BranchPhase::Rollback
    } else {
        BranchPhase::Commit
    };
    let expected = if rollback {
        GlobalStatus::Rollbacked
    } else {
        GlobalStatus::Committed
    };
    if let Some(mode) = &fault {
        assert!(matches!(
            mode.as_str(),
            "duplicate" | "reconnect" | "restart" | "crash"
        ));
    }
    let end = if rollback {
        global.rollback().await
    } else {
        global.commit().await
    };
    let observed = match end {
        Ok(TransactionCompletion::Observed { outcome, .. }) => Some(outcome.status()),
        other if fault.is_some() => {
            let _ = other;
            None
        }
        other => panic!("original XA global end unresolved: {other:?}"),
    };
    if fault.is_none() {
        assert_eq!(
            observed,
            Some(expected),
            "original Java TC did not confirm requested XA global direction"
        );
    }
    // 故障下只等待原 TC 自身的重试；客户端不重发 End，也不从 Finished 猜终态。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(25);
    loop {
        let remaining:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM global_table WHERE xid=?)+(SELECT COUNT(*) FROM branch_table WHERE xid=?)").bind(xid.as_str()).bind(xid.as_str()).fetch_one(&mut admin).await.unwrap();
        if remaining == 0 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original TC did not remove original global/branch after retry"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let state = store.lease(identity.clone(), phase).await.unwrap().state();
    assert_eq!(state, Some(BranchRecordState::Completed));
    let rows = fixture::business(&mut admin).await;
    assert_eq!(rows, if rollback { vec![] } else { vec![(1, 42)] });
    assert!(sqlx::raw_sql("XA RECOVER")
        .fetch_all(&mut admin)
        .await
        .unwrap()
        .is_empty());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM global_table WHERE xid=?)+(SELECT COUNT(*) FROM branch_table WHERE xid=?)+(SELECT COUNT(*) FROM lock_table WHERE xid=?)").bind(xid.as_str()).bind(xid.as_str()).bind(xid.as_str()).fetch_one(&mut admin).await.unwrap();
        if remaining == 0 {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    if let Some(mode) = fault {
        fixture::record(
            "original-java-xa-fault",
            serde_json::json!({"mode":mode,"xid":xid.as_str(),"branch_id":identity.branch_id().to_string(),"resource_id":adapter.key().resource_id(),"branch_type":3,"lock_key":null,"application_data":null,"phase":if rollback{"Rollback"}else{"Commit"},"initial_global_status":observed.map(|v|format!("{v:?}")),"expected_tc_terminal":format!("{expected:?}"),"journal":"Completed","business":rows,"prepared_remaining":0,"phase_one_reports":0,"client_end_requests":1,"finished_inferred_as_success":false}),
        );
    } else {
        fixture::record(
            "original-java-xa",
            serde_json::json!({"xid":xid.as_str(),"branch_id":identity.branch_id().to_string(),"resource_id":adapter.key().resource_id(),"branch_type":3,"lock_key":null,"application_data":null,"phase":if rollback{"Rollback"}else{"Commit"},"global_status":format!("{:?}",observed.unwrap()),"journal":"Completed","business":rows,"prepared_remaining":0,"phase_one_reports":0}),
        );
    }
    rm.shutdown().await.unwrap();
    tm.shutdown().await.unwrap();
    source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires pinned original Java TC and owned MySQL"]
async fn original_java_tc_commits_managed_xa_original_branch() {
    run(false).await;
}
#[tokio::test]
#[ignore = "requires pinned original Java TC and owned MySQL"]
async fn original_java_tc_rolls_back_managed_xa_original_branch() {
    run(true).await;
}
