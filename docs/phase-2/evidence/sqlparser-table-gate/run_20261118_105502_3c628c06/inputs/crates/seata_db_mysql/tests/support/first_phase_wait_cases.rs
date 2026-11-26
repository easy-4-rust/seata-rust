//! 实际控制行／Undo 唯一键锁等待中的取消与绝对期限。
use super::{first_fixture, fixture, owned_mysql, recovery};
use first_fixture::Harness;
use seata_at::MySqlJdbcProfile;
use seata_db_mysql::{
    DbError, MySqlDataSource, MySqlDataSourceManager, MySqlOptions, MySqlPrepareError,
    MySqlRecoveryError,
};
use seata_rm::BranchResource;
use sqlx::Connection;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn preparation_resource_mismatch_is_local_before_registration() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    let database = std::env::var("SEATA_MYSQL_DATABASE").unwrap() + "_other";
    assert!(
        database.starts_with("undo_")
            && database
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    sqlx::query(&format!("CREATE DATABASE `{database}`"))
        .execute(&mut h.admin)
        .await
        .unwrap();
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
        database,
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap();
    let other = MySqlDataSource::connect(options).await.unwrap();
    let adapter =
        MySqlDataSourceManager::new(other.clone(), MySqlJdbcProfile::Connector8Default).unwrap();
    assert_ne!(adapter.key(), h.adapter.key());
    let tx = h.transaction().await;
    assert!(matches!(
        adapter
            .prepare_transaction(tx, &h.rm.client(), "prepare:wrong-resource", None)
            .await,
        Err(MySqlPrepareError::Local {
            source: DbError::Input { .. }
        })
    ));
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
    other.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn actual_enrollment_and_undo_waits_preserve_known_receipt_and_cancelled_scope() {
    let mut evidence = Vec::new();
    for enrollment in [true, false] {
        for cancel in [true, false] {
            let mut h = Harness::start(
                if cancel {
                    Duration::from_secs(3)
                } else {
                    Duration::from_millis(1500)
                },
                Duration::from_secs(5),
            )
            .await;
            let mut socket = h.accept().await;
            let mut blocker = owned_mysql::admin().await;
            let mut guard = blocker.begin().await.unwrap();
            if enrollment {
                sqlx::query(
                    "SELECT xid FROM seata_rs_at_branch_v1 WHERE xid=? AND branch_id=42 FOR UPDATE",
                )
                .bind(b"prepare:wait".as_slice())
                .fetch_all(&mut *guard)
                .await
                .unwrap();
            } else {
                sqlx::query("INSERT INTO undo_log VALUES(42,'prepare:wait','serializer=jackson&compressorType=NONE',?,1,now(6),now(6))").bind(b"{}".as_slice()).execute(&mut *guard).await.unwrap();
            }
            let tx = h.transaction().await;
            let adapter = h.adapter.clone();
            let client = h.rm.client();
            let started = std::time::Instant::now();
            let task = tokio::spawn(async move {
                adapter
                    .prepare_transaction(
                        tx,
                        &client,
                        "prepare:wait",
                        Some("prepare-fixture-app".into()),
                    )
                    .await
            });
            let wire = first_fixture::request(&mut socket, "prepare:wait").await;
            first_fixture::positive(&mut socket, wire).await;
            let table = if enrollment {
                "seata_rs_at_branch_v1"
            } else {
                "undo_log"
            };
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            let waits = loop {
                let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME=?").bind(table).fetch_one(&mut h.admin).await.unwrap();
                if count > 0 {
                    break count;
                }
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "actual preparation lock wait required"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
            };
            if cancel {
                task.abort();
                assert!(task.await.unwrap_err().is_cancelled());
            } else {
                let error = task.await.unwrap().unwrap_err();
                assert!(
                    matches!(
                        &error,
                        MySqlPrepareError::Deadline { .. }
                            | MySqlPrepareError::Enrollment {
                                source: MySqlRecoveryError::BeforeCommit {
                                    source: DbError::Deadline { .. },
                                    ..
                                },
                                ..
                            }
                            | MySqlPrepareError::Undo {
                                source: DbError::Deadline { .. },
                                ..
                            }
                    ),
                    "actual error={error:?}, enrollment={enrollment}"
                );
                assert!(
                    started.elapsed() < Duration::from_millis(2000),
                    "preparation must share the entry deadline"
                );
                assert_eq!(
                    error.registered_branch().unwrap().identity().branch_id(),
                    42
                );
                assert!(!error.is_registration_unknown());
            }
            guard.rollback().await.unwrap();
            let facts = h.closed_counts(&mut socket).await;
            assert_eq!(
                facts,
                (0, 0, if enrollment { (0, 0, 0) } else { (1, 0, 0) })
            );
            evidence.push(serde_json::json!({"blocked_table":table,"cancelled":cancel,"actual_waits":waits,"elapsed_ms":started.elapsed().as_millis(),"receipt_when_deadline":if cancel {None} else {Some(42)},"business_rows":facts.0,"undo_rows":facts.1,"recovery":facts.2}));
        }
    }
    fixture::save("prepare-lock-waits.json", &serde_json::json!(evidence));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn an_unpolled_preparation_future_drops_the_original_transaction_without_registering() {
    let mut h = Harness::start(Duration::from_secs(3), Duration::from_secs(2)).await;
    let mut socket = h.accept().await;
    let tx = h.transaction().await;
    let client = h.rm.client();
    let future = h
        .adapter
        .prepare_transaction(tx, &client, "prepare:unpolled", None);
    drop(future);
    assert_eq!(recovery::state(&mut h.admin).await, (0, 0, 0));
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
}
