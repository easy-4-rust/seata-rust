//! 持久 RM 终态与补偿同事务的真实 COMMIT 故障。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/rm_recovery_fixture.rs"]
#[allow(dead_code)]
mod recovery;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;
#[path = "support/undo_rollback_fixture.rs"]
mod rollback_fixture;
use seata_db_mysql::MySqlDataSource;
use seata_rm::{BranchPhase, BranchResolution, BranchResource};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned MySQL and original packet recovery commit observer"]
async fn lost_terminal_commit_recovers_without_replaying_compensation_or_business() {
    let (mut admin, seed) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        seata_at::MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    seed.close().await.unwrap();
    recovery::schema(&mut admin).await;
    let port = std::env::var("SEATA_RM_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let mode = std::env::var("SEATA_ROLLBACK_FAULT").unwrap();
    let source = MySqlDataSource::connect(
        owned_mysql::options(Some(port))
            .operation_timeout(Duration::from_millis(1500))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter = recovery::adapter(source);
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    assert_eq!(
        adapter
            .apply(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Unknown,
        "unconfirmed control COMMIT must remain Unknown"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    let before = (
        fixture::count(&mut admin, "undo_work").await,
        fixture::count(&mut admin, "undo_log").await,
        recovery::state(&mut admin).await,
    );
    assert_eq!(
        before,
        if mode == "block_commit" {
            (1, 1, (1, 2, 0))
        } else {
            (0, 0, (1, 2, 1))
        }
    );
    assert_eq!(
        adapter
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    let after = (
        fixture::count(&mut admin, "undo_work").await,
        fixture::count(&mut admin, "undo_log").await,
        recovery::state(&mut admin).await,
    );
    assert_eq!(after, (0, 0, (1, 2, 1)));
    fixture::save(
        "rm-wire-observation.json",
        &serde_json::json!({"mode":mode,"before":before,"after":after,"xid":receipt.identity().xid(),"branch_id":receipt.identity().branch_id(),"resource_id":receipt.identity().resource().resource_id(),"first":"Unknown","recovered":"Applied","business_replayed":false}),
    );
}
