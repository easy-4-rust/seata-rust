//! 独立 Undo API 的原结果事件与物理 COMMIT 尝试边界。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/java_checksum_events.rs"]
mod capture;
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/control_commit_proxy.rs"]
mod proxy;
#[path = "support/undo_rollback_fixture.rs"]
mod rollback_fixture;
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{MySqlDataSource, UndoFenceObservation, UndoRollbackObservation};
use sha2::{Digest, Sha256};
use sqlx::Connection;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

fn selected(events: &Arc<Mutex<Vec<BTreeMap<String, String>>>>) -> Vec<BTreeMap<String, String>> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.get("target").is_some_and(|t| t == "seata_db_mysql::undo"))
        .cloned()
        .collect()
}
fn check(events: &[BTreeMap<String, String>], resource: &str) {
    let digest = format!("{:x}", Sha256::digest(resource.as_bytes()));
    fixture::save(
        &format!("undo-context-{digest}.json"),
        &serde_json::json!({"resource_id":resource,"xid":"rollback:xid","branch_id":42}),
    );
    let fields = [
        "target",
        "message",
        "operation",
        "xid",
        "branch_id",
        "resource_sha256",
        "outcome",
        "observation",
        "commit_attempted",
    ];
    for e in events {
        assert_eq!(e.len(), fields.len());
        assert!(fields.iter().all(|f| e.contains_key(*f)));
        assert_eq!(e["message"], "mysql_undo_outcome");
        assert_eq!(e["xid"], "rollback:xid");
        assert_eq!(e["branch_id"], "42");
        assert_eq!(e["resource_sha256"], digest);
        for secret in [
            "undo-event-secret",
            "jdbc:mysql",
            "seata-mysql-fixture-only",
            "INSERT INTO",
        ] {
            assert!(!format!("{e:?}").contains(secret));
        }
    }
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn original_undo_observations_and_read_only_repeats_keep_actual_commit_boundary() {
    let (mut admin, source) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'undo-event-secret')"],
        MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let _capture = tracing::subscriber::set_default(capture::EventCapture(events.clone()));
    drop(source.fence_undo_log("rollback:xid", 42));
    assert!(selected(&events).is_empty());
    assert_eq!(
        source.fence_undo_log("rollback:xid", 42).await.unwrap(),
        UndoFenceObservation::UndoPresent
    );
    assert_eq!(
        selected(&events).len(),
        1,
        "missing original Undo result event"
    );
    assert_eq!(
        source
            .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Consumed {
            logs: 1,
            restored_rows: 1
        }
    );
    assert_eq!(
        source.fence_undo_log("rollback:xid", 42).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: true }
    );
    assert_eq!(
        source.fence_undo_log("rollback:xid", 42).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: false }
    );
    assert_eq!(
        source
            .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
            .await
            .unwrap(),
        UndoRollbackObservation::Fenced { inserted: false }
    );
    assert!(source.fence_undo_log("", 0).await.is_err());
    assert!(source
        .rollback_undo_log("", 0, MySqlJdbcProfile::Connector8Default)
        .await
        .is_err());
    let values = selected(&events);
    check(&values, &source.resource_id().unwrap());
    let boundaries: Vec<_> = values
        .iter()
        .map(|e| {
            (
                e["operation"].as_str(),
                e["outcome"].as_str(),
                e["observation"].as_str(),
                e["commit_attempted"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        boundaries,
        vec![
            ("fence", "completed", "undo_present", "false"),
            ("rollback", "completed", "consumed", "true"),
            ("fence", "completed", "fenced_inserted", "true"),
            ("fence", "completed", "fenced_existing", "false"),
            ("rollback", "completed", "fenced_existing", "false")
        ],
        "original Undo result or COMMIT boundary changed"
    );
    let business = fixture::count(&mut admin, "undo_work").await;
    let undo = fixture::count(&mut admin, "undo_log").await;
    assert_eq!((business, undo), (0, 1));
    source.close().await.unwrap();
    fixture::save(
        "undo-events-observations.json",
        &serde_json::json!({"events":values,"business":business,"undo":undo}),
    );
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn actual_undo_row_wait_cancel_and_closed_pool_do_not_attempt_commit() {
    for rollback in [false, true] {
        let (mut admin, source) = rollback_fixture::seed(
            &[],
            &["INSERT INTO undo_work(id,v) VALUES(1,'undo-event-secret')"],
            MySqlJdbcProfile::Connector8Default,
            None,
        )
        .await;
        let mut observer = owned_mysql::admin().await;
        let mut lock = admin.begin().await.unwrap();
        sqlx::query("SELECT xid FROM undo_log FOR UPDATE")
            .fetch_all(&mut *lock)
            .await
            .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let _capture = tracing::subscriber::set_default(capture::EventCapture(events.clone()));
        {
            let future = async {
                if rollback {
                    source
                        .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
                        .await
                        .map(|_| ())
                        .map_err(|e| format!("{e:?}"))
                } else {
                    source
                        .fence_undo_log("rollback:xid", 42)
                        .await
                        .map(|_| ())
                        .map_err(|e| format!("{e:?}"))
                }
            };
            tokio::pin!(future);
            tokio::select! {r=&mut future=>panic!("Undo wait returned early {r:?}"),()=async{tokio::time::timeout(Duration::from_secs(2),async{loop{let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='undo_log'").fetch_one(&mut observer).await.unwrap();if n>0{break;}tokio::time::sleep(Duration::from_millis(10)).await;}}).await.unwrap();}=>{}}
        }
        lock.rollback().await.unwrap();
        source.close().await.unwrap();
        if rollback {
            assert!(source
                .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
                .await
                .is_err());
        } else {
            assert!(source.fence_undo_log("rollback:xid", 42).await.is_err());
        }
        let values = selected(&events);
        check(&values, &source.resource_id().unwrap());
        assert_eq!(values.len(), 2, "Undo cancellation event missing");
        assert_eq!(values[0]["outcome"], "cancelled");
        assert_eq!(values[1]["outcome"], "before_commit_error");
        assert!(values
            .iter()
            .all(|e| e["commit_attempted"] == "false" && e["observation"] == "not_observed"));
        let business = fixture::count(&mut admin, "undo_work").await;
        let undo = fixture::count(&mut admin, "undo_log").await;
        assert_eq!((business, undo), (1, 1));
        fixture::save(
            if rollback {
                "undo-events-rollback-wait.json"
            } else {
                "undo-events-fence-wait.json"
            },
            &serde_json::json!({"events":values,"business":business,"undo":undo}),
        );
    }
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn actual_undo_commit_ok_held_remains_unknown_or_cancelled_then_original_identity_recovers() {
    for rollback in [false, true] {
        for cancelled in [false, true] {
            let (mut admin, seed) = rollback_fixture::seed(
                &[],
                &["INSERT INTO undo_work(id,v) VALUES(1,'undo-event-secret')"],
                MySqlJdbcProfile::Connector8Default,
                None,
            )
            .await;
            seed.close().await.unwrap();
            let proxy = proxy::ControlCommitProxy::start(
                std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
            )
            .await;
            let source = MySqlDataSource::connect(
                owned_mysql::options(Some(proxy.port))
                    .operation_timeout(Duration::from_secs(4))
                    .unwrap(),
            )
            .await
            .unwrap();
            let events = Arc::new(Mutex::new(Vec::new()));
            let _capture = tracing::subscriber::set_default(capture::EventCapture(events.clone()));
            proxy.arm();
            let mut pending = Box::pin(async {
                if rollback {
                    source
                        .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.is_unknown_outcome())
                } else {
                    source
                        .fence_undo_log("rollback:xid", 42)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.is_unknown_outcome())
                }
            });
            // fence 在 Normal 存在时只读；本场景先删除原 Undo，保留业务数据以验证围栏不补偿。
            // 删除发生在首次轮询前，因而不改变被测调用中的数据库操作。
            if !rollback {
                sqlx::query("DELETE FROM undo_log")
                    .execute(&mut admin)
                    .await
                    .unwrap();
            }
            tokio::select! {r=&mut pending=>panic!("held Undo COMMIT returned early {r:?}"),()=async{tokio::time::timeout(Duration::from_secs(3),async{while proxy.captured().is_none(){tokio::time::sleep(Duration::from_millis(5)).await;}}).await.unwrap();}=>{}}
            let before = (
                fixture::count(&mut admin, "undo_work").await,
                fixture::count(&mut admin, "undo_log").await,
            );
            assert_eq!(before, if rollback { (0, 0) } else { (1, 1) });
            if cancelled {
                drop(pending);
            } else {
                assert_eq!(pending.await, Err(true));
            }
            let first = selected(&events);
            assert_eq!(first.len(), 1, "Undo unknown/cancelled event missing");
            assert_eq!(
                first[0]["outcome"],
                if cancelled {
                    "cancelled"
                } else {
                    "unknown_commit"
                }
            );
            assert_eq!(first[0]["observation"], "not_observed");
            assert_eq!(
                first[0]["commit_attempted"], "true",
                "actual Undo COMMIT marker missing"
            );
            if rollback {
                assert_eq!(
                    source
                        .rollback_undo_log("rollback:xid", 42, MySqlJdbcProfile::Connector8Default)
                        .await
                        .unwrap(),
                    UndoRollbackObservation::Fenced { inserted: true }
                );
            } else {
                assert_eq!(
                    source.fence_undo_log("rollback:xid", 42).await.unwrap(),
                    UndoFenceObservation::Fenced { inserted: false }
                );
            }
            let values = selected(&events);
            check(&values, &source.resource_id().unwrap());
            assert_eq!(values.len(), 2);
            assert_eq!(values[1]["outcome"], "completed");
            assert_eq!(
                values[1]["observation"],
                if rollback {
                    "fenced_inserted"
                } else {
                    "fenced_existing"
                }
            );
            assert_eq!(
                values[1]["commit_attempted"],
                if rollback { "true" } else { "false" }
            );
            let after = (
                fixture::count(&mut admin, "undo_work").await,
                fixture::count(&mut admin, "undo_log").await,
            );
            assert_eq!(after, if rollback { (0, 1) } else { (1, 1) });
            source.close().await.unwrap();
            let (request, ok) = proxy.captured().unwrap();
            fixture::save(
                &format!(
                    "undo-events-{}-{}.json",
                    if rollback { "rollback" } else { "fence" },
                    if cancelled { "cancelled" } else { "unknown" }
                ),
                &serde_json::json!({"events":values,"actual_commit_packet":request,"actual_ok_packet":ok,"before_recovery":before,"after_recovery":after}),
            );
            proxy.shutdown().await;
        }
    }
}
