//! 完整 JDBC 码的真实 MySQL 准入与原事务共享锁验收。
#![allow(clippy::unwrap_used)]
use super::MySqlResourceBinding;
use crate::DbError;
use seata_at::MySqlJdbcProfile;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
use std::{path::PathBuf, time::Duration};
#[tokio::test]
#[ignore = "requires owned MySQL; run jdbc_profile_admission_java.py"]
async fn full_jdbc_profile_matrix_and_shared_lock() {
    let database = std::env::var("SEATA_MYSQL_DATABASE").unwrap();
    assert!(database.starts_with("undo_jdbc_profile_"));
    let port: u16 = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let options = MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username("root")
        .password(&std::env::var("SEATA_MYSQL_PASSWORD").unwrap())
        .database(&database)
        .ssl_mode(MySqlSslMode::Disabled);
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .unwrap();
    let resource = format!("jdbc:mysql://127.0.0.1:{port}/{database}");
    sqlx::query("DROP TABLE IF EXISTS seata_rs_at_resource_binding")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/005_seata_rs_at_resource_binding.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO seata_rs_at_resource_binding VALUES(1,?)")
        .bind(resource.as_bytes())
        .execute(&pool)
        .await
        .unwrap();
    let mut old = pool.begin().await.unwrap();
    MySqlResourceBinding::lock_verified(&mut old, &resource, None, false)
        .await
        .unwrap();
    old.rollback().await.unwrap();
    let mut absent = pool.begin().await.unwrap();
    assert!(matches!(
        MySqlResourceBinding::lock_verified(
            &mut absent,
            &resource,
            Some(MySqlJdbcProfile::Connector8ExactTemporalUtc),
            true
        )
        .await,
        Err(DbError::Driver {
            code: Some(1054),
            ..
        })
    ));
    absent.rollback().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/011_seata_rs_at_tinyint_profile_binding.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE seata_rs_at_resource_binding SET tinyint1_is_bit=1")
        .execute(&pool)
        .await
        .unwrap();
    let mut tiny = pool.begin().await.unwrap();
    MySqlResourceBinding::lock_verified(
        &mut tiny,
        &resource,
        Some(MySqlJdbcProfile::Connector8ExactTemporalUtc),
        false,
    )
    .await
    .unwrap();
    tiny.rollback().await.unwrap();
    let mut missing = pool.begin().await.unwrap();
    assert!(matches!(
        MySqlResourceBinding::lock_verified(
            &mut missing,
            &resource,
            Some(MySqlJdbcProfile::Connector8ExactTemporalUtc),
            true
        )
        .await,
        Err(DbError::Driver {
            code: Some(1054),
            ..
        })
    ));
    missing.rollback().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/021_seata_rs_at_jdbc_profile_binding.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let value: Option<u8> =
        sqlx::query_scalar("SELECT jdbc_profile FROM seata_rs_at_resource_binding WHERE slot=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(value.is_none());
    let mut null = pool.begin().await.unwrap();
    assert!(matches!(
        MySqlResourceBinding::lock_verified(
            &mut null,
            &resource,
            Some(MySqlJdbcProfile::Connector8ExactTemporalUtc),
            true
        )
        .await,
        Err(DbError::Input {
            reason: "MySQL AT JDBC profile binding is missing or differs"
        })
    ));
    null.rollback().await.unwrap();
    let mut accepted = 0;
    let mut rejected = 0;
    for code in 1..=6 {
        let stored = MySqlJdbcProfile::from_storage_code(code).unwrap();
        sqlx::query(
            "UPDATE seata_rs_at_resource_binding SET tinyint1_is_bit=?,jdbc_profile=? WHERE slot=1",
        )
        .bind(u8::from(stored.boolean_tinyint()))
        .bind(code)
        .execute(&pool)
        .await
        .unwrap();
        for requested in 1..=6 {
            let mut tx = pool.begin().await.unwrap();
            let result = MySqlResourceBinding::lock_verified(
                &mut tx,
                &resource,
                Some(MySqlJdbcProfile::from_storage_code(requested).unwrap()),
                true,
            )
            .await;
            if code == requested {
                result.unwrap();
                accepted += 1;
            } else {
                assert!(matches!(result, Err(DbError::Input { .. })));
                rejected += 1;
            }
            tx.rollback().await.unwrap();
        }
    }
    assert_eq!((accepted, rejected), (6, 30));
    let invalid =
        sqlx::query("UPDATE seata_rs_at_resource_binding SET jdbc_profile=7 WHERE slot=1")
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(
        invalid
            .as_database_error()
            .and_then(|e| e.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
            .map(|e| e.number()),
        Some(3819)
    );
    sqlx::query(
        "UPDATE seata_rs_at_resource_binding SET tinyint1_is_bit=1,jdbc_profile=5 WHERE slot=1",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut guard = pool.begin().await.unwrap();
    MySqlResourceBinding::lock_verified(
        &mut guard,
        &resource,
        Some(MySqlJdbcProfile::Connector8ExactTemporalUtc),
        true,
    )
    .await
    .unwrap();
    let (send, ready) = tokio::sync::oneshot::channel();
    let updater_pool = pool.clone();
    let update = tokio::spawn(async move {
        let mut connection = updater_pool.acquire().await.unwrap();
        let id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        send.send(id).unwrap();
        sqlx::query("UPDATE seata_rs_at_resource_binding SET jdbc_profile=3 WHERE slot=1")
            .execute(&mut *connection)
            .await
            .unwrap()
            .rows_affected()
    });
    let updater_id = ready.await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let waiting:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.threads t ON w.REQUESTING_THREAD_ID=t.THREAD_ID WHERE t.PROCESSLIST_ID=?").bind(updater_id).fetch_one(&pool).await.unwrap();
        if waiting > 0 {
            assert!(!update.is_finished());
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    guard.rollback().await.unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(10), update)
            .await
            .unwrap()
            .unwrap(),
        1
    );
    let declared: u8 =
        sqlx::query_scalar("SELECT jdbc_profile FROM seata_rs_at_resource_binding WHERE slot=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(declared, 3);
    let out = PathBuf::from(std::env::var("SEATA_JDBC_PROFILE_GUARD_OUT").unwrap());
    std::fs::write(out.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"JDBC_PROFILE_MATRIX_AND_SHARED_LOCK_VERIFIED","accepted":accepted,"rejected":rejected,"legacy_005_without_new_columns":true,"legacy_011_without_full_profile":true,"missing_columns_rejected":2,"migration_preserved_null":true,"null_rejected":true,"unknown_code_constraint_rejected":true,"unknown_code_mysql_error":3819,"actual_waiting_updater_connection":updater_id,"shared_lock_wait_observed":true,"update_completed_after_rollback":true,"final_profile":declared})).unwrap()).unwrap();
    pool.close().await;
}
