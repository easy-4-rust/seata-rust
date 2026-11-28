//! 数据库控制事务的实际结果与克隆作用域；需运行器自有 MySQL。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
pub(crate) mod fixture;
#[path = "support/owned_mysql.rs"]
pub(crate) mod owned_mysql;
#[path = "support/rm_recovery_fixture.rs"]
#[allow(dead_code)]
pub(crate) mod recovery;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
pub(crate) mod rm_fixture;
#[path = "support/undo_rollback_fixture.rs"]
pub(crate) mod rollback_fixture;
use seata_db_mysql::{MySqlDataSource, MySqlRecoveryOperation};
use seata_rm::{BranchPhase, BranchResolution, BranchResource};
use sqlx::Connection;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn shared_control_counts_follow_durable_results_and_duplicate_terminal_reads() {
    let (mut admin, ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        seata_at::MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    recovery::schema(&mut admin).await;
    let sampler = ds.recovery_metrics_sampler();
    let adapter = recovery::adapter(ds.clone());
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    let e = sampler.snapshot(MySqlRecoveryOperation::Enroll);
    assert_eq!(
        (e.completed, e.unresolved, e.applied, e.active),
        (1, 1, 0, 0)
    );
    assert_eq!(
        adapter
            .apply(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    let b = sampler.snapshot(MySqlRecoveryOperation::BindRollback);
    let r = sampler.snapshot(MySqlRecoveryOperation::Rollback);
    assert_eq!((b.completed, b.unresolved, b.applied), (1, 1, 0));
    assert_eq!((r.completed, r.unresolved, r.applied), (1, 0, 1));
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM undo_work")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    let replacement = recovery::adapter(ds.clone());
    assert_eq!(
        replacement
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(
        replacement
            .recovery_metrics_sampler()
            .snapshot(MySqlRecoveryOperation::Rollback)
            .applied,
        2
    );
    assert_eq!(
        sampler
            .snapshot(MySqlRecoveryOperation::BindRollback)
            .applied,
        1
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    ds.close().await.unwrap();
    drop(adapter);
    drop(replacement);
    drop(ds);
    assert_eq!(
        sampler.snapshot(MySqlRecoveryOperation::Rollback).completed,
        2
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn blocked_control_cancel_is_separate_from_before_commit_error_and_keeps_no_effect() {
    let mut admin = fixture::reset().await;
    recovery::schema(&mut admin).await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = recovery::adapter(ds.clone());
    let sampler = adapter.recovery_metrics_sampler();
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    let mut observer = owned_mysql::admin().await;
    let mut locked = admin.begin().await.unwrap();
    sqlx::query("SELECT xid FROM seata_rs_at_branch_v1 FOR UPDATE")
        .fetch_all(&mut *locked)
        .await
        .unwrap();
    {
        let future = adapter.enroll(&receipt);
        tokio::pin!(future);
        tokio::select! {
            result=&mut future=>panic!("control unexpectedly completed: {result:?}"),
            ()=async {
                tokio::time::timeout(Duration::from_secs(2),async {
                    loop {
                        let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits").fetch_one(&mut observer).await.unwrap();
                        if waits>0 {break;}
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await.expect("actual InnoDB lock wait required before cancellation");
            }=>{}
        }
        assert_eq!(sampler.snapshot(MySqlRecoveryOperation::Enroll).active, 1);
        // 丢弃整个作用域中的 Future 后才允许观察 cancelled。
    }
    drop(locked);
    ds.close().await.unwrap();
    let error = adapter.enroll(&receipt).await.unwrap_err();
    assert!(!error.is_unknown_outcome());
    let final_count = sampler.snapshot(MySqlRecoveryOperation::Enroll);
    assert_eq!(
        (
            final_count.active,
            final_count.completed,
            final_count.before_commit_error,
            final_count.unknown_commit,
            final_count.cancelled
        ),
        (0, 1, 1, 0, 1)
    );
    assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
}
