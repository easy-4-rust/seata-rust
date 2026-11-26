//! 独立原始MySQL包观察者；正式调用仅见断连/期限，不知道夹具如何处理COMMIT。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use seata_db_mysql::{DbError, MySqlDataSource, UndoFenceError, UndoFenceObservation};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned MySQL and raw packet observer"]
async fn unknown_control_commit_requires_original_identity_observation() {
    let mut admin = fixture::reset().await;
    let port = std::env::var("SEATA_FENCE_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let ds = MySqlDataSource::connect(
        owned_mysql::options(Some(port))
            .operation_timeout(Duration::from_millis(750))
            .unwrap(),
    )
    .await
    .unwrap();
    let error = ds.fence_undo_log("fence:wire", 4242).await.unwrap_err();
    assert!(matches!(error, UndoFenceError::CommitOutcomeUnknown { .. }));
    assert!(error.is_unknown_outcome());
    assert_eq!(error.identity(), Some(("fence:wire", 4242)));
    assert!(!format!("{error:?} {error}").contains("fence:wire"));
    let mode = std::env::var("SEATA_FENCE_FAULT").unwrap();
    if mode == "delay_ok" {
        assert!(matches!(
            error,
            UndoFenceError::CommitOutcomeUnknown {
                source: DbError::Deadline { .. },
                ..
            }
        ));
    }
    // 等断连服务端完成回滚；仅夹具检查已知故障事实，正式API从未接收mode。
    tokio::time::sleep(Duration::from_millis(100)).await;
    let before = fixture::count(&mut admin, "undo_log").await;
    assert_eq!(before, i64::from(mode != "block_commit"));
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    let observation = ds.fence_undo_log("fence:wire", 4242).await.unwrap();
    assert_eq!(
        observation,
        UndoFenceObservation::Fenced {
            inserted: mode == "block_commit"
        }
    );
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 1);
    let status: i32 = sqlx::query_scalar(
        "SELECT log_status FROM undo_log WHERE xid='fence:wire' AND branch_id=4242",
    )
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(status, 1);
    fixture::save(
        "wire-observation.json",
        &serde_json::json!({"mode":mode,"error":format!("{error:?}"),"unknown":error.is_unknown_outcome(),"xid":error.identity().unwrap().0,"branch_id":error.identity().unwrap().1,"before_explicit_recovery":before,"recovery_inserted":mode=="block_commit","after":1,"business_rows":0}),
    );
    ds.close().await.unwrap();
}
