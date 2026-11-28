//! 恢复控制事件只陈述本次调用结果，不把终态观察当成重复业务效果。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/control_commit_proxy.rs"]
mod control_commit_proxy;
#[path = "support/java_checksum_events.rs"]
mod event_capture;
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
use seata_rm::{BranchPhase, BranchResolution, BranchResource};
use sha2::{Digest, Sha256};
use sqlx::Connection;
use std::time::Duration;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

fn selected(events: &Arc<Mutex<Vec<BTreeMap<String, String>>>>) -> Vec<BTreeMap<String, String>> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter(|row| {
            row.get("target")
                .is_some_and(|target| target == "seata_db_mysql::control")
        })
        .cloned()
        .collect()
}
fn check(events: &[BTreeMap<String, String>], resource: &str) {
    let digest = format!("{:x}", Sha256::digest(resource.as_bytes()));
    fixture::save(
        &format!("control-event-context-{digest}.json"),
        &serde_json::json!({"resource_id":resource,"xid":"rollback:xid","branch_id":42}),
    );
    let fields = [
        "target",
        "message",
        "operation",
        "requested_phase",
        "xid",
        "branch_id",
        "resource_sha256",
        "outcome",
        "resolution",
        "commit_attempted",
    ];
    for event in events {
        assert_eq!(event.len(), fields.len());
        assert!(fields.iter().all(|name| event.contains_key(*name)));
        assert_eq!(event["target"], "seata_db_mysql::control");
        assert_eq!(event["message"], "mysql_control_outcome");
        assert_eq!(event["xid"], "rollback:xid");
        assert_eq!(event["branch_id"], "42");
        assert_eq!(
            event["resource_sha256"],
            format!("{:x}", Sha256::digest(resource.as_bytes()))
        );
        assert_eq!(
            event["requested_phase"],
            if event["operation"] == "enroll" {
                "none"
            } else {
                "rollback"
            }
        );
        let text = format!("{event:?}");
        for secret in [
            "event-secret-value",
            "seata-mysql-fixture-only",
            "INSERT INTO",
            "jdbc:mysql",
            "application-data-private",
        ] {
            assert!(!text.contains(secret));
        }
    }
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn real_control_writes_and_duplicate_reads_emit_distinct_commit_boundaries() {
    let (mut admin, source) = rollback_fixture::seed(
        &[],
        &["INSERT INTO undo_work(id,v) VALUES(1,'event-secret-value')"],
        seata_at::MySqlJdbcProfile::Connector8Default,
        None,
    )
    .await;
    recovery::schema(&mut admin).await;
    let adapter = recovery::adapter(source.clone());
    let receipt = recovery::receipt(adapter.clone(), Some("application-data-private")).await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let _capture = tracing::subscriber::set_default(event_capture::EventCapture(events.clone()));
    let never = adapter.enroll(&receipt);
    drop(never);
    assert!(selected(&events).is_empty());
    adapter.enroll(&receipt).await.unwrap();
    let values = selected(&events);
    assert_eq!(values.len(), 1, "missing real MySQL control event");
    assert_eq!(values[0]["operation"], "enroll");
    assert_eq!(values[0]["outcome"], "completed");
    assert_eq!(values[0]["resolution"], "unresolved");
    assert_eq!(
        values[0]["commit_attempted"], "true",
        "actual COMMIT attempt marker missing"
    );
    assert_eq!(
        adapter
            .apply(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(
        adapter
            .recover(receipt.identity(), BranchPhase::Rollback)
            .await
            .unwrap(),
        BranchResolution::Applied
    );
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(recovery::state(&mut admin).await, (1, 2, 1));
    source.close().await.unwrap();
    let values = selected(&events);
    check(&values, adapter.key().resource_id());
    let boundaries: Vec<_> = values
        .iter()
        .map(|e| {
            (
                e["operation"].as_str(),
                e["outcome"].as_str(),
                e["resolution"].as_str(),
                e["commit_attempted"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        boundaries,
        vec![
            ("enroll", "completed", "unresolved", "true"),
            ("bind_rollback", "completed", "unresolved", "true"),
            ("rollback", "completed", "applied", "true"),
            ("bind_rollback", "completed", "applied", "false"),
            ("rollback", "completed", "applied", "false")
        ]
    );
    fixture::save(
        "control-events-success.json",
        &serde_json::to_value(&values).unwrap(),
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn actual_row_wait_cancellation_and_closed_pool_keep_commit_unattempted() {
    let mut admin = fixture::reset().await;
    recovery::schema(&mut admin).await;
    let source = seata_db_mysql::MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = recovery::adapter(source.clone());
    let receipt = recovery::receipt(adapter.clone(), None).await;
    adapter.enroll(&receipt).await.unwrap();
    let mut observer = owned_mysql::admin().await;
    let mut locked = admin.begin().await.unwrap();
    sqlx::query("SELECT xid FROM seata_rs_at_branch_v1 FOR UPDATE")
        .fetch_all(&mut *locked)
        .await
        .unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let _capture = tracing::subscriber::set_default(event_capture::EventCapture(events.clone()));
    {
        let future = adapter.enroll(&receipt);
        tokio::pin!(future);
        tokio::select! {
            value=&mut future=>panic!("control row wait completed early {value:?}"),
            ()=async {
                tokio::time::timeout(Duration::from_secs(2),async {loop {
                    let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='seata_rs_at_branch_v1'").fetch_one(&mut observer).await.unwrap();
                    if waits>0 {break;};tokio::time::sleep(Duration::from_millis(10)).await;
                }}).await.unwrap();
            }=>{}
        }
    }
    locked.rollback().await.unwrap();
    source.close().await.unwrap();
    assert!(adapter.enroll(&receipt).await.is_err());
    assert_eq!(recovery::state(&mut admin).await, (1, 0, 0));
    let values = selected(&events);
    check(&values, adapter.key().resource_id());
    assert_eq!(values.len(), 2, "control cancellation event missing");
    assert_eq!(values[0]["outcome"], "cancelled");
    assert_eq!(values[1]["outcome"], "before_commit_error");
    assert!(values
        .iter()
        .all(|e| e["commit_attempted"] == "false" && e["resolution"] == "not_observed"));
    fixture::save(
        "control-events-before-commit.json",
        &serde_json::to_value(values).unwrap(),
    );
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn original_commit_ok_held_is_unknown_or_cancelled_then_same_branch_recovers() {
    for cancelled in [false, true] {
        let (mut admin, seed_source) = rollback_fixture::seed(
            &[],
            &["INSERT INTO undo_work(id,v) VALUES(1,'event-secret-value')"],
            seata_at::MySqlJdbcProfile::Connector8Default,
            None,
        )
        .await;
        recovery::schema(&mut admin).await;
        seed_source.close().await.unwrap();
        let proxy = control_commit_proxy::ControlCommitProxy::start(
            std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        )
        .await;
        let source = seata_db_mysql::MySqlDataSource::connect(
            owned_mysql::options(Some(proxy.port))
                .operation_timeout(Duration::from_secs(4))
                .unwrap(),
        )
        .await
        .unwrap();
        let adapter = recovery::adapter(source.clone());
        let receipt = recovery::receipt(adapter.clone(), None).await;
        let events = Arc::new(Mutex::new(Vec::new()));
        let _capture =
            tracing::subscriber::set_default(event_capture::EventCapture(events.clone()));
        proxy.arm();
        let mut pending = Box::pin(adapter.enroll(&receipt));
        tokio::select! {
            value=&mut pending=>panic!("held control COMMIT returned before capture {value:?}"),
            ()=async {tokio::time::timeout(Duration::from_secs(3),async{while proxy.captured().is_none(){tokio::time::sleep(Duration::from_millis(5)).await;}}).await.unwrap();}=>{}
        }
        let before_recovery = recovery::state(&mut admin).await;
        assert_eq!(
            before_recovery,
            (1, 0, 0),
            "original control write must actually be durable before cancelling/timeout"
        );
        if cancelled {
            drop(pending);
        } else {
            assert!(pending.await.unwrap_err().is_unknown_outcome());
        }
        let values = selected(&events);
        check(&values, adapter.key().resource_id());
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0]["outcome"],
            if cancelled {
                "cancelled"
            } else {
                "unknown_commit"
            }
        );
        assert_eq!(values[0]["resolution"], "not_observed");
        assert_eq!(
            values[0]["commit_attempted"], "true",
            "actual COMMIT attempt marker missing"
        );
        assert_eq!(
            adapter
                .apply(receipt.identity(), BranchPhase::Rollback)
                .await
                .unwrap(),
            BranchResolution::Applied
        );
        let after_recovery = recovery::state(&mut admin).await;
        let business_rows = fixture::count(&mut admin, "undo_work").await;
        assert_eq!(after_recovery, (1, 2, 1));
        assert_eq!(business_rows, 0);
        source.close().await.unwrap();
        let values = selected(&events);
        check(&values, adapter.key().resource_id());
        assert_eq!(values.len(), 3);
        assert_eq!(values[1]["operation"], "bind_rollback");
        assert_eq!(values[2]["operation"], "rollback");
        assert_eq!(values[2]["resolution"], "applied");
        let (request, ok) = proxy.captured().unwrap();
        let hex = |bytes: Vec<u8>| {
            let digits = b"0123456789abcdef";
            let mut value = String::with_capacity(bytes.len() * 2);
            for byte in bytes {
                value.push(char::from(digits[usize::from(byte >> 4)]));
                value.push(char::from(digits[usize::from(byte & 15)]));
            }
            value
        };
        fixture::save(
            if cancelled {
                "control-events-cancelled-commit.json"
            } else {
                "control-events-unknown-commit.json"
            },
            &serde_json::json!({"events":values,"actual_commit_packet":hex(request),"actual_ok_packet":hex(ok),"before_recovery":before_recovery,"after_recovery":after_recovery,"business_rows":business_rows}),
        );
        proxy.shutdown().await;
    }
}
