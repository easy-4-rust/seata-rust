//! 真实失败边界：拥有式事务退出、权限、累计日志预算及临时表清理。
use super::{admin, options};
use seata_at::SqlRecognizerFactory;
use seata_datasource::{FieldValue, ImageError};
use seata_db_mysql::{DbError, MySqlDataSource, MySqlOptions};
use std::time::Duration;
#[path = "query_barrier_proxy.rs"]
mod query_barrier_proxy;

async fn old_value() -> i32 {
    sqlx::query_scalar("SELECT v FROM execution_work WHERE id=1")
        .fetch_one(&mut admin().await)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn missing_temp_permission_poisons_without_committing_prior_business() {
    let config = MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        "execution_no_temp".into(),
        std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
        std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap();
    let ds = MySqlDataSource::connect(config).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let update =
        SqlRecognizerFactory::recognize("UPDATE execution_work SET v=11 WHERE id=1").unwrap();
    tx.execute(&update, &[]).await.unwrap();
    let insert = SqlRecognizerFactory::recognize(
        "INSERT INTO execution_rounding(id,note) VALUES(2,'new-two')",
    )
    .unwrap();
    let result = tx.execute(&insert, &[]).await;
    assert!(
        matches!(
            result,
            Err(DbError::Driver {
                code: Some(1044),
                ..
            })
        ),
        "expected real CREATE TEMPORARY permission denial: {result:?}"
    );
    assert_eq!(tx.undo_logs().len(), 1);
    assert!(matches!(
        tx.table_meta("execution_work").await,
        Err(DbError::Poisoned)
    ));
    assert_eq!(old_value().await, 10);
    tx.rollback().await.unwrap();
    assert_eq!(old_value().await, 10);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn invalid_key_assignment_leaves_no_committed_effect_and_session_exit_cleans_temp() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let update =
        SqlRecognizerFactory::recognize("UPDATE execution_work SET v=11 WHERE id=1").unwrap();
    tx.execute(&update, &[]).await.unwrap();
    let insert = SqlRecognizerFactory::recognize(
        "INSERT INTO execution_rounding(id,note) VALUES(NULL,'bad')",
    )
    .unwrap();
    assert!(matches!(
        tx.execute(&insert, &[]).await,
        Err(DbError::Driver {
            code: Some(1048),
            ..
        })
    ));
    assert_eq!(tx.undo_logs().len(), 1);
    assert!(matches!(
        tx.execute(&update, &[]).await,
        Err(DbError::Poisoned)
    ));
    assert_eq!(old_value().await, 10);
    let mut observer = admin().await;
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.INNODB_TEMP_TABLE_INFO")
            .fetch_one(&mut observer)
            .await
            .unwrap();
    assert_eq!(
        remaining, 1,
        "failed assignment left the created session-local table"
    );
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    wait_clean(&mut observer).await;
    assert_eq!(old_value().await, 10);
}

async fn wait_clean(observer: &mut sqlx::MySqlConnection) {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM information_schema.INNODB_TEMP_TABLE_INFO",
            )
            .fetch_one(&mut *observer)
            .await
            .unwrap();
            if count == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("actual server session must clean temporary objects without proxy shutdown");
}

async fn cancel_at_query(prefix: &[u8], statement: &str, expect_temporary: bool, expire: bool) {
    let port = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let proxy = query_barrier_proxy::QueryBarrierProxy::start(port, prefix.to_vec()).await;
    let ds = MySqlDataSource::connect(
        options(Some(proxy.port))
            .operation_timeout(Duration::from_secs(if expire { 3 } else { 10 }))
            .unwrap(),
    )
    .await
    .unwrap();
    let mut tx = ds.begin().await.unwrap();
    if expect_temporary {
        let update =
            SqlRecognizerFactory::recognize("UPDATE execution_work SET v=11 WHERE id=1").unwrap();
        tx.execute(&update, &[]).await.unwrap();
    }
    let sql = SqlRecognizerFactory::recognize(statement).unwrap();
    let mut executing = Box::pin(tx.execute(&sql, &[]));
    tokio::select! {
        result=&mut executing=>panic!("execution escaped its actual query barrier: {result:?}"),
        result=tokio::time::timeout(Duration::from_secs(4),async {while !proxy.captured(){tokio::time::sleep(Duration::from_millis(5)).await;}})=>result.unwrap(),
    }
    let raw = proxy.captured_packet().unwrap();
    assert_eq!(raw[4], 0x16);
    assert!(raw[4..].starts_with(prefix));
    println!(
        "cancel_query_packet_hex={}",
        raw.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let mut observer = admin().await;
    assert_eq!(
        old_value().await,
        10,
        "independent MVCC reader sees uncommitted work only as old data"
    );
    sqlx::query("SET SESSION innodb_lock_wait_timeout=1")
        .execute(&mut observer)
        .await
        .unwrap();
    let lock = tokio::select! {
        result=&mut executing=>panic!("execution escaped lock proof: {result:?}"),
        result=sqlx::query("UPDATE execution_work SET v=v WHERE id=1").execute(&mut observer)=>result,
    };
    assert_eq!(
        lock.unwrap_err()
            .as_database_error()
            .unwrap()
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .unwrap()
            .number(),
        1205
    );
    if expect_temporary {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.INNODB_TEMP_TABLE_INFO")
                .fetch_one(&mut observer)
                .await
                .unwrap();
        assert_eq!(count, 1, "barrier follows real temporary key assignment");
    }
    if expire {
        assert!(matches!(
            executing.await,
            Err(DbError::Deadline {
                operation: "business and images"
            })
        ));
    } else {
        drop(executing);
    }
    assert!(matches!(
        tx.table_meta("execution_work").await,
        Err(DbError::Poisoned)
    ));
    // 先放行原始包，保持代理存活；不能由销毁代理代替生产连接的取消清理。
    proxy.release();
    wait_clean(&mut observer).await;
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let result = sqlx::query("UPDATE execution_work SET v=v WHERE id=1")
                .execute(&mut observer)
                .await;
            match result {
                Ok(result) => {
                    assert_eq!(result.rows_affected(), 1);
                    break;
                }
                Err(error) => {
                    assert_eq!(
                        error
                            .as_database_error()
                            .unwrap()
                            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
                            .unwrap()
                            .number(),
                        1205
                    );
                }
            }
        }
    })
    .await
    .expect("cancellation must release actual prior write lock before proxy shutdown");
    assert_eq!(
        old_value().await,
        10,
        "cancelled original update is rolled back"
    );
    assert_eq!(tx.undo_logs().len(), usize::from(expect_temporary));
    ds.close().await.unwrap();
    println!("cancel_cleaned_before_proxy_shutdown=true prior_write_rolled_back=true temporary={expect_temporary}");
    proxy.shutdown().await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn cancellation_after_business_before_after_image_releases_the_actual_write() {
    cancel_at_query(
        b"\x16SELECT `id`, `v`, `note`, `payload` FROM `execution_work` WHERE (`id` = ?)",
        "UPDATE execution_work SET v=11 WHERE v=10",
        false,
        false,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn cancellation_after_key_assignment_cleans_temp_and_rolls_back_prior_business() {
    cancel_at_query(
        b"\x16SELECT `k0`, `r` FROM `__seata_at_keys_",
        "INSERT INTO execution_rounding(id,note) VALUES(2,'new-two')",
        true,
        false,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn deadline_after_key_assignment_cleans_temp_and_rolls_back_prior_business() {
    cancel_at_query(
        b"\x16SELECT `k0`, `r` FROM `__seata_at_keys_",
        "INSERT INTO execution_rounding(id,note) VALUES(2,'new-two')",
        true,
        true,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn business_table_matching_next_internal_name_is_not_shadowed_or_dropped() {
    let mut observer = admin().await;
    sqlx::query("SET GLOBAL log_output='TABLE'")
        .execute(&mut observer)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=ON")
        .execute(&mut observer)
        .await
        .unwrap();
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut first = ds.begin().await.unwrap();
    let insert = SqlRecognizerFactory::recognize(
        "INSERT INTO execution_rounding(id,note) VALUES(2,'new-two')",
    )
    .unwrap();
    first.execute(&insert, &[]).await.unwrap();
    let create:String=sqlx::query_scalar("SELECT CAST(argument AS CHAR CHARACTER SET utf8mb4) FROM mysql.general_log WHERE command_type='Execute' AND argument LIKE 'CREATE TEMPORARY TABLE %' ORDER BY event_time DESC LIMIT 1").fetch_one(&mut observer).await.unwrap();
    let name = create
        .strip_prefix("CREATE TEMPORARY TABLE `__seata_at_keys_")
        .unwrap()
        .split('`')
        .next()
        .unwrap();
    let next = u64::from_str_radix(name, 16).unwrap() + 1;
    let business = format!("__seata_at_keys_{next:x}");
    first.rollback().await.unwrap();
    sqlx::query(&format!(
        "CREATE TABLE `{business}` (id INT PRIMARY KEY,note VARCHAR(50)) ENGINE=InnoDB"
    ))
    .execute(&mut observer)
    .await
    .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize(&format!(
        "INSERT INTO `{business}`(id,note) VALUES(7,'real-business')"
    ))
    .unwrap();
    assert_eq!(tx.execute(&sql, &[]).await.unwrap(), 1);
    let image = tx.undo_logs()[0].after_image();
    assert_eq!(image.table_name(), business);
    assert_eq!(
        image.rows()[0].field("note").unwrap().value(),
        &FieldValue::Text("real-business".into())
    );
    let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM `{business}`"))
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "the real table exists and business is still uncommitted"
    );
    let temporary:String=sqlx::query_scalar("SELECT CAST(argument AS CHAR CHARACTER SET utf8mb4) FROM mysql.general_log WHERE command_type='Execute' AND argument LIKE 'CREATE TEMPORARY TABLE %' ORDER BY event_time DESC LIMIT 1").fetch_one(&mut observer).await.unwrap();
    assert!(!temporary.starts_with(&format!("CREATE TEMPORARY TABLE `{business}` ")));
    tx.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM `{business}`"))
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "real table must survive DROP TEMPORARY and rollback"
    );
    sqlx::query(&format!("DROP TABLE `{business}`"))
        .execute(&mut observer)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut observer)
        .await
        .unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn cumulative_undo_payload_limit_poisons_and_rolls_back_all_statements() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql =
        SqlRecognizerFactory::recognize("UPDATE execution_work SET payload=? WHERE id=1").unwrap();
    for n in 0..8 {
        tx.execute(&sql, &[FieldValue::Binary(vec![n; 1_000_000])])
            .await
            .unwrap();
    }
    assert_eq!(tx.undo_logs().len(), 8);
    assert!(matches!(
        tx.execute(&sql, &[FieldValue::Binary(vec![9; 1_000_000])])
            .await,
        Err(DbError::Image(ImageError::Budget { .. }))
    ));
    assert_eq!(tx.undo_logs().len(), 8);
    assert!(matches!(
        tx.table_meta("execution_work").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let payload: Vec<u8> = sqlx::query_scalar("SELECT payload FROM execution_work WHERE id=1")
        .fetch_one(&mut admin().await)
        .await
        .unwrap();
    assert_eq!(payload, vec![b'x']);
    ds.close().await.unwrap();
}
