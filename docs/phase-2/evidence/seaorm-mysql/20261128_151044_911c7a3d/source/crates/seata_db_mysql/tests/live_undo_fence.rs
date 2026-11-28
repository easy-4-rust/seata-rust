//! 真实原GlobalFinished围栏；不以空查询证明旧业务不生效，不替代二阶段结果分类。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use seata_at::{JacksonUndoLogParser, MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::UndoFenceError;
use seata_db_mysql::{DbError, MySqlDataSource, UndoFenceObservation};
use sqlx::{Connection, Row};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn durable_guard_blocks_two_original_late_normal_writers_without_replaying_business() {
    let mut admin = fixture::reset().await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut first = ds.begin().await.unwrap();
    let mut second = ds.begin().await.unwrap();
    for (tx, id) in [(&mut first, 1), (&mut second, 2)] {
        tx.execute(
            &SqlRecognizerFactory::recognize(&format!(
                "INSERT INTO undo_work(id,v) VALUES({id},'once')"
            ))
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
    }
    let payload = JacksonUndoLogParser::default()
        .encode_prepared(
            "fence:xid",
            42,
            first.undo_logs(),
            MySqlJdbcProfile::Connector8Default,
        )
        .unwrap();
    let observation = ds.fence_undo_log("fence:xid", 42).await.unwrap();
    assert_eq!(observation, UndoFenceObservation::Fenced { inserted: true });
    let row=sqlx::query("SELECT xid,branch_id,log_status,rollback_info,context,CAST(log_created AS CHAR) AS created,CAST(log_modified AS CHAR) AS modified FROM undo_log").fetch_one(&mut admin).await.unwrap();
    assert_eq!(row.get::<String, _>("xid"), "fence:xid");
    assert_eq!(row.get::<i64, _>("branch_id"), 42);
    assert_eq!(row.get::<i32, _>("log_status"), 1);
    assert_eq!(row.get::<Vec<u8>, _>("rollback_info"), b"{}");
    let created: String = row.get("created");
    assert_eq!(created, row.get::<String, _>("modified"));
    fixture::save(
        "global-finished.json",
        &serde_json::json!({"xid":"fence:xid","branch_id":42,"context":row.get::<String,_>("context"),"bytes":row.get::<Vec<u8>,_>("rollback_info"),"status":1,"log_created":created,"log_modified":row.get::<String,_>("modified")}),
    );
    fixture::save(
        "late-normal-payload.json",
        &serde_json::json!({"bytes":payload}),
    );
    for mut tx in [first, second] {
        assert!(matches!(
            tx.flush_undo_logs("fence:xid", 42, MySqlJdbcProfile::Connector8Default)
                .await,
            Err(DbError::Driver {
                code: Some(1062),
                ..
            })
        ));
        assert!(matches!(
            tx.table_meta("undo_work").await,
            Err(DbError::Poisoned)
        ));
        tx.rollback().await.unwrap();
    }
    assert_eq!(fixture::count(&mut admin, "undo_work").await, 0);
    assert_eq!(
        ds.fence_undo_log("fence:xid", 42).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: false }
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT CAST(log_created AS CHAR) FROM undo_log")
            .fetch_one(&mut admin)
            .await
            .unwrap(),
        created
    );
    ds.close().await.unwrap();
    let reopened = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    assert_eq!(
        reopened.fence_undo_log("fence:xid", 42).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: false }
    );
    reopened.close().await.unwrap();
}

async fn snapshot(admin: &mut sqlx::MySqlConnection) -> serde_json::Value {
    let rows=sqlx::query("SELECT branch_id,xid,context,rollback_info,log_status,CAST(log_created AS CHAR) AS created,CAST(log_modified AS CHAR) AS modified FROM undo_log ORDER BY branch_id").fetch_all(admin).await.unwrap();
    serde_json::json!(rows.iter().map(|r|serde_json::json!({"branch_id":r.get::<i64,_>("branch_id"),"xid":r.get::<String,_>("xid"),"context":r.get::<String,_>("context"),"bytes":r.get::<Vec<u8>,_>("rollback_info"),"status":r.get::<i32,_>("log_status"),"created":r.get::<String,_>("created"),"modified":r.get::<String,_>("modified")})).collect::<Vec<_>>())
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn committed_normal_is_preserved_as_opaque_pending_work() {
    let mut admin = fixture::reset().await;
    sqlx::query("INSERT INTO undo_log VALUES(42,'fence:xid','original opaque context',X'01020304',0,now(6),now(6))").execute(&mut admin).await.unwrap();
    let before = snapshot(&mut admin).await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    assert_eq!(
        ds.fence_undo_log("fence:xid", 42).await.unwrap(),
        UndoFenceObservation::UndoPresent
    );
    assert_eq!(before, snapshot(&mut admin).await);
    fixture::save("normal-preserved.json", &before);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn malformed_or_unknown_guard_never_becomes_a_fence() {
    let mut admin = fixture::reset().await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    for (status, context, bytes) in [
        (
            2,
            "serializer=jackson&compressorType=NONE",
            b"{}".as_slice(),
        ),
        (1, "serializer=other&compressorType=NONE", b"{}".as_slice()),
        (
            1,
            "serializer=jackson&compressorType=NONE&extra=1",
            b"{}".as_slice(),
        ),
        (
            1,
            "serializer=jackson&compressorType=NONE",
            b"[]".as_slice(),
        ),
        (
            1,
            "serializer=jackson&compressorType=NONE",
            b"huge".as_slice(),
        ),
    ] {
        sqlx::query("DELETE FROM undo_log")
            .execute(&mut admin)
            .await
            .unwrap();
        sqlx::query("INSERT INTO undo_log VALUES(42,'fence:xid',?,?,?,now(6),now(6))")
            .bind(context)
            .bind(bytes)
            .bind(status)
            .execute(&mut admin)
            .await
            .unwrap();
        let before = snapshot(&mut admin).await;
        assert!(matches!(
            ds.fence_undo_log("fence:xid", 42).await,
            Err(UndoFenceError::BeforeCommit {
                source: DbError::Unsupported {
                    feature: "Undo fence existing guard shape"
                        | "Undo fence existing guard content"
                },
                ..
            })
        ));
        assert_eq!(before, snapshot(&mut admin).await);
    }
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn invalid_identity_has_no_sql_effect_and_debug_hides_xid() {
    let mut admin = fixture::reset().await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    for (xid, id) in [("", 42), ("abc", 0), ("abc", -1), (&"a".repeat(129), 42)] {
        let error = ds.fence_undo_log(xid, id).await.unwrap_err();
        assert!(matches!(error, UndoFenceError::InvalidIdentity));
        assert_eq!(error.identity(), None);
        assert!(!error.is_unknown_outcome());
    }
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    sqlx::query("DROP TABLE undo_log")
        .execute(&mut admin)
        .await
        .unwrap();
    let error = ds
        .fence_undo_log("private-xid-value", 42)
        .await
        .unwrap_err();
    assert_eq!(error.identity(), Some(("private-xid-value", 42)));
    assert!(!error.is_unknown_outcome());
    assert!(!format!("{error:?} {error}").contains("private-xid-value"));
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn relaxed_flush_configuration_is_rejected_without_guard() {
    let mut admin = fixture::reset().await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    sqlx::query("SET GLOBAL innodb_flush_log_at_trx_commit=2")
        .execute(&mut admin)
        .await
        .unwrap();
    let result = ds.fence_undo_log("fence:xid", 42).await;
    sqlx::query("SET GLOBAL innodb_flush_log_at_trx_commit=1")
        .execute(&mut admin)
        .await
        .unwrap();
    assert!(matches!(
        result,
        Err(UndoFenceError::BeforeCommit {
            source: DbError::Unsupported { .. },
            ..
        })
    ));
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    ds.close().await.unwrap();
}

async fn wait_fence<
    F: std::future::Future<Output = Result<UndoFenceObservation, UndoFenceError>>,
>(
    future: &mut std::pin::Pin<Box<F>>,
    admin: &mut sqlx::MySqlConnection,
    name: &str,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    loop {
        tokio::select! {r=future.as_mut()=>panic!("fence ended before actual lock wait: {r:?}"),_=tokio::time::sleep(Duration::from_millis(5))=>{}}
        let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='undo_log'").fetch_one(&mut *admin).await.unwrap();
        if waits > 0 {
            fixture::save(
                name,
                &serde_json::json!({"waits":waits,"xid":"fence:xid","branch_id":42}),
            );
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "actual original identity wait missing"
        );
    }
}
async fn blocked_control(cancel: bool) {
    let mut admin = fixture::reset().await;
    let mut owner = owned_mysql::admin().await;
    let mut blocker = owner.begin().await.unwrap();
    sqlx::query("INSERT INTO undo_log VALUES(42,'fence:xid','original',X'7B7D',0,now(6),now(6))")
        .execute(&mut *blocker)
        .await
        .unwrap();
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
    let mut future = Box::pin(ds.fence_undo_log("fence:xid", 42));
    wait_fence(
        &mut future,
        &mut admin,
        if cancel {
            "cancel-lock-wait.json"
        } else {
            "deadline-lock-wait.json"
        },
    )
    .await;
    if cancel {
        drop(future);
    } else {
        assert!(matches!(
            future.await,
            Err(UndoFenceError::BeforeCommit {
                source: DbError::Deadline { .. },
                ..
            })
        ));
    }
    blocker.rollback().await.unwrap();
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 0);
    assert_eq!(
        ds.fence_undo_log("fence:xid", 42).await.unwrap(),
        UndoFenceObservation::Fenced { inserted: true }
    );
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn cancellation_during_actual_identity_wait_publishes_no_guard() {
    blocked_control(true).await;
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn deadline_during_actual_identity_wait_stays_before_commit() {
    blocked_control(false).await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn original_normal_winner_is_observed_after_actual_lock_arbitration() {
    let mut admin = fixture::reset().await;
    let mut owner = owned_mysql::admin().await;
    let mut blocker = owner.begin().await.unwrap();
    sqlx::query("INSERT INTO undo_log VALUES(42,'fence:xid','original',X'7B7D',0,now(6),now(6))")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut future = Box::pin(ds.fence_undo_log("fence:xid", 42));
    wait_fence(&mut future, &mut admin, "normal-winner-wait.json").await;
    blocker.commit().await.unwrap();
    assert_eq!(future.await.unwrap(), UndoFenceObservation::UndoPresent);
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 1);
    fixture::save("normal-winner.json", &snapshot(&mut admin).await);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn two_control_attempts_have_one_durable_identity_without_automatic_retry() {
    let mut admin = fixture::reset().await;
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        ds.fence_undo_log("fence:xid", 42),
        ds.fence_undo_log("fence:xid", 42)
    );
    let results = [a, b];
    fixture::save(
        "concurrent-outcomes.json",
        &serde_json::json!({"results":results.iter().map(|r|format!("{r:?}")).collect::<Vec<_>>()}),
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Ok(UndoFenceObservation::Fenced { inserted: true })))
            .count(),
        1
    );
    for result in results {
        assert!(
            matches!(
                result,
                Ok(UndoFenceObservation::Fenced { .. })
                    | Err(UndoFenceError::BeforeCommit {
                        source: DbError::Driver {
                            code: Some(1213 | 1062),
                            ..
                        },
                        ..
                    })
            ),
            "unexpected concurrent outcome: {result:?}"
        );
    }
    assert_eq!(fixture::count(&mut admin, "undo_log").await, 1);
    fixture::save("concurrent-guard.json", &snapshot(&mut admin).await);
    ds.close().await.unwrap();
}
