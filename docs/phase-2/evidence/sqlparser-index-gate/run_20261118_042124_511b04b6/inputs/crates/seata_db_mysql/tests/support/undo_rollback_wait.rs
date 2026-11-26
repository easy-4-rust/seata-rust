//! 补偿操作在真实原Undo锁等待中的取消/期限夹具。
use super::{fixture, owned_mysql, rollback_fixture};
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{DbError, MySqlDataSource, UndoRollbackError, UndoRollbackObservation};
use sqlx::Connection;
use std::time::Duration;
pub async fn blocked_rollback(cancel: bool) {
    let (mut admin, seed_ds) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'created')"],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    seed_ds.close().await.unwrap();
    let before = rollback_fixture::snapshot(&mut admin).await;
    let ds = MySqlDataSource::connect(
        owned_mysql::options(None)
            .operation_timeout(if cancel {
                Duration::from_secs(3)
            } else {
                Duration::from_millis(1500)
            })
            .unwrap(),
    )
    .await
    .unwrap();
    let mut owner = owned_mysql::admin().await;
    let mut lock = owner.begin().await.unwrap();
    sqlx::query(
        "SELECT branch_id FROM undo_log WHERE xid='rollback:xid' AND branch_id=42 FOR UPDATE",
    )
    .fetch_one(&mut *lock)
    .await
    .unwrap();
    let mut future =
        Box::pin(ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        tokio::select! {result=future.as_mut()=>panic!("rollback ended before physical lock wait: {result:?}"),_=tokio::time::sleep(Duration::from_millis(5))=>{}}
        let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='undo_log'").fetch_one(&mut admin).await.unwrap();
        if waits > 0 {
            fixture::save(
                if cancel {
                    "cancel-wait.json"
                } else {
                    "deadline-wait.json"
                },
                &serde_json::json!({"actual_waits":waits,"xid":"rollback:xid","branch_id":42}),
            );
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "actual Undo lock wait missing"
        );
    }
    if cancel {
        drop(future);
    } else {
        assert!(matches!(
            future.await,
            Err(UndoRollbackError::BeforeCommit {
                source: DbError::Deadline { .. },
                ..
            })
        ));
    }
    lock.rollback().await.unwrap();
    assert_eq!(before, rollback_fixture::snapshot(&mut admin).await);
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 1);
    assert_eq!(
        ds.rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 1,
            restored_rows: 1
        }
    );
    ds.close().await.unwrap();
}
