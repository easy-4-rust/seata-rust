//! 固定原 Java TC 的正式 TCC 全局事务与真实同库 Fence/业务。
#![cfg(feature = "tcc")]
#![allow(clippy::unwrap_used)]
#[path = "support/tcc_managed_fixture.rs"]
mod fixture;
use seata_protocol::GlobalStatus;
use seata_rm::{
    BranchPhase, BranchRecordState, BranchResource, FileBranchStore, JournalOptions,
    ResourceCatalog, ResourceManager,
};
use seata_tm::{TransactionCompletion, TransactionConfig, TransactionManager};
use std::time::Duration;

async fn run(rollback: bool) {
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
    .request_timeout(Duration::from_secs(5))
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
    let config = TransactionConfig::builder("native-managed-tcc".into())
        .transaction_timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let mut global = tm.begin(&config).await.unwrap();
    let app = fixture::app(1);
    let receipt = adapter
        .register_and_try(&rm.client(), global.xid().as_str(), Some(app.clone()))
        .await
        .unwrap();
    let identity = receipt.identity().clone();
    assert_eq!(receipt.lock_key(), None);
    assert_eq!(fixture::business(&mut admin).await, vec![(1, 7)]);
    let result = if rollback {
        global.rollback().await.unwrap()
    } else {
        global.commit().await.unwrap()
    };
    let TransactionCompletion::Observed { outcome, .. } = result else {
        panic!("original TCC global end unresolved")
    };
    assert_eq!(
        outcome.status(),
        if rollback {
            GlobalStatus::Rollbacked
        } else {
            GlobalStatus::Committed
        }
    );
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
    assert_eq!(business, if rollback { vec![] } else { vec![(1, 14)] });
    let status: u8 = sqlx::query_scalar("SELECT status FROM seata_rs_tcc_fence_v1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(status, if rollback { 3 } else { 2 });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM global_table)+(SELECT COUNT(*) FROM branch_table)+(SELECT COUNT(*) FROM lock_table)").fetch_one(&mut admin).await.unwrap();
        if remaining == 0 {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    fixture::record(
        "original-java-tcc",
        serde_json::json!({"xid":identity.xid(),"branch_id":identity.branch_id().to_string(),"resource_id":adapter.key().resource_id(),"branch_type":identity.resource().branch_type().to_java_v1(),"application_data":app,"lock_key":null,"global_status":format!("{:?}",outcome.status()),"status":status,"journal":"Completed","business":business,"finished_inferred_as_success":false}),
    );
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
