//! 必须由自有真实MySQL运行器显式执行，普通工作区回归不冒充现场。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_at::SqlRecognizerFactory;
use seata_datasource::{DecimalValue, FieldValue, ImageError};
use seata_db_mysql::{DbError, MySqlDataSource};
use std::time::Duration;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/stall_proxy.rs"]
mod stall_proxy;
use owned_mysql::{admin, options};

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn update_before_image_uses_where_parameters_and_preserves_full_exact_rows() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize("UPDATE account SET note=? WHERE a>=?").unwrap();
    let result = tx
        .before_image(
            &sql,
            &[
                FieldValue::Text("not-executed".into()),
                FieldValue::Signed(1),
            ],
        )
        .await;
    assert!(result.is_ok());
    let records = result.unwrap();
    assert_eq!(records.rows().len(), 2);
    assert_eq!(records.table_meta().columns().len(), 6);
    let first = &records.rows()[0];
    let second = &records.rows()[1];
    assert_eq!(first.field("a").unwrap().value(), &FieldValue::Signed(1));
    assert_eq!(second.field("a").unwrap().value(), &FieldValue::Signed(2));
    assert_eq!(
        first.field("b").unwrap().value(),
        &FieldValue::Signed(i64::MIN)
    );
    assert_eq!(
        second.field("b").unwrap().value(),
        &FieldValue::Signed(i64::MAX)
    );
    assert_eq!(
        first.field("unsigned_id").unwrap().value(),
        &FieldValue::Unsigned(u64::MAX)
    );
    assert_eq!(
        second.field("unsigned_id").unwrap().value(),
        &FieldValue::Unsigned(9_007_199_254_740_993)
    );
    assert_eq!(
        first.field("amount").unwrap().value(),
        &FieldValue::Decimal(
            DecimalValue::new("12345678901234567890123456789012345.123456789012345678901234567890")
                .unwrap()
        )
    );
    assert_eq!(
        second.field("amount").unwrap().value(),
        &FieldValue::Decimal(DecimalValue::new("-0.000000000000000000000000000001").unwrap())
    );
    assert_eq!(
        first.field("payload").unwrap().value(),
        &FieldValue::Binary(vec![0, 255, 127, 0])
    );
    assert_eq!(second.field("payload").unwrap().value(), &FieldValue::Null);
    assert_eq!(second.field("note").unwrap().value(), &FieldValue::Null);
    let mut independent = admin().await;
    let subset = tx
        .before_image(&sql, &[FieldValue::Text("0".into()), FieldValue::Signed(2)])
        .await;
    assert!(subset.is_ok());
    let subset = subset.unwrap();
    assert_eq!(subset.rows().len(), 1);
    assert_eq!(
        subset.rows()[0].field("a").unwrap().value(),
        &FieldValue::Signed(2)
    );
    let note: String = sqlx::query_scalar("SELECT note FROM account WHERE a=1")
        .fetch_one(&mut independent)
        .await
        .unwrap();
    assert_eq!(note, "fixture");
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn decimal_and_unsigned_predicate_parameters_keep_native_numeric_semantics() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql =
        SqlRecognizerFactory::recognize("DELETE FROM account WHERE unsigned_id=? AND amount=?")
            .unwrap();
    let records = tx
        .before_image(
            &sql,
            &[
                FieldValue::Unsigned(u64::MAX),
                FieldValue::Decimal(
                    DecimalValue::new(
                        "12345678901234567890123456789012345.123456789012345678901234567890",
                    )
                    .unwrap(),
                ),
            ],
        )
        .await
        .unwrap();
    assert_eq!(records.rows().len(), 1);
    let numeric = SqlRecognizerFactory::recognize("DELETE FROM account WHERE ? < ?").unwrap();
    let records = tx
        .before_image(
            &numeric,
            &[
                FieldValue::Decimal(DecimalValue::new("2").unwrap()),
                FieldValue::Decimal(DecimalValue::new("10").unwrap()),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        records.rows().len(),
        2,
        "DECIMAL parameters must compare numerically, not lexically"
    );
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn physical_types_decode_char_padding_binary_null_and_exact_unsigned_values() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize("DELETE FROM physical_types WHERE id=?").unwrap();
    let records = tx
        .before_image(&sql, &[FieldValue::Signed(1)])
        .await
        .unwrap();
    assert_eq!(records.rows().len(), 1);
    let row = &records.rows()[0];
    for (name, expected) in [
        ("s8", -128),
        ("s16", -32768),
        ("s24", -8388608),
        ("s32", -2147483648),
    ] {
        assert_eq!(
            row.field(name).unwrap().value(),
            &FieldValue::Signed(expected)
        );
    }
    for (name, expected) in [
        ("u8", 255),
        ("u16", 65535),
        ("u24", 16777215),
        ("u32", 4294967295),
    ] {
        assert_eq!(
            row.field(name).unwrap().value(),
            &FieldValue::Unsigned(expected)
        );
    }
    assert_eq!(
        row.field("txt").unwrap().value(),
        &FieldValue::Text("🦀".into())
    );
    assert_eq!(
        row.field("txt3").unwrap().value(),
        &FieldValue::Text("中文".into())
    );
    assert_eq!(
        row.field("fixed_text").unwrap().value(),
        &FieldValue::Text("x".into())
    );
    assert_eq!(
        row.field("fixed_binary").unwrap().value(),
        &FieldValue::Binary(vec![255, 0, 0, 0])
    );
    assert_eq!(
        row.field("unsigned_decimal").unwrap().value(),
        &FieldValue::Decimal(DecimalValue::new("999.99").unwrap())
    );
    assert_eq!(
        row.field("nullable_col").unwrap().value(),
        &FieldValue::Null
    );
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn empty_images_keep_schema_and_bad_parameter_counts_do_not_poison() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize("DELETE FROM account WHERE a=?").unwrap();
    assert!(matches!(
        tx.before_image(&sql, &[]).await,
        Err(DbError::Input { .. })
    ));
    let records = tx
        .before_image(&sql, &[FieldValue::Signed(999)])
        .await
        .unwrap();
    assert!(records.rows().is_empty());
    assert_eq!(records.table_name(), "account");
    assert_eq!(records.table_meta().columns().len(), 6);
    tx.table_meta("account").await.unwrap();
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn row_field_and_total_budgets_reject_before_full_payload_select() {
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
    for table in ["image_bigfield", "image_bytebudget", "image_rowbudget"] {
        let mut tx = ds.begin().await.unwrap();
        let sql = SqlRecognizerFactory::recognize(&format!("DELETE FROM {table}")).unwrap();
        assert!(
            matches!(
                tx.before_image(&sql, &[]).await,
                Err(DbError::Image(ImageError::Budget { .. }))
            ),
            "expected preflight budget for {table}"
        );
        let full_pattern = format!("SELECT `id`, `payload` FROM `{table}`%");
        let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Execute' AND argument LIKE ?").bind(full_pattern).fetch_one(&mut observer).await.unwrap();
        assert_eq!(
            count, 0,
            "full payload query executed before budget rejection for {table}"
        );
        assert!(matches!(tx.table_meta(table).await, Err(DbError::Poisoned)));
        tx.rollback().await.unwrap();
    }
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut observer)
        .await
        .unwrap();
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn captured_rows_stay_locked_until_original_transaction_rolls_back() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize("DELETE FROM image_lock_guard WHERE id=?").unwrap();
    let image = tx
        .before_image(&sql, &[FieldValue::Signed(1)])
        .await
        .unwrap();
    assert_eq!(image.rows().len(), 1);
    let mut other = admin().await;
    sqlx::query("SET SESSION innodb_lock_wait_timeout=1")
        .execute(&mut other)
        .await
        .unwrap();
    let error = sqlx::query("UPDATE image_lock_guard SET v=11 WHERE id=1")
        .execute(&mut other)
        .await
        .unwrap_err();
    assert_eq!(
        error
            .as_database_error()
            .unwrap()
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .unwrap()
            .number(),
        1205
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query("UPDATE image_lock_guard SET v=11 WHERE id=1")
            .execute(&mut other)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    let value: i32 = sqlx::query_scalar("SELECT v FROM image_lock_guard WHERE id=1")
        .fetch_one(&mut other)
        .await
        .unwrap();
    assert_eq!(value, 11);
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn caller_cancellation_releases_the_owned_transaction_before_proxy_shutdown() {
    let port = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let proxy = stall_proxy::StallProxy::start(port).await;
    let ds = MySqlDataSource::connect(options(Some(proxy.port)))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.table_meta("image_cancel").await.unwrap();
    let mut observer = admin().await;
    sqlx::query("SET SESSION lock_wait_timeout=1")
        .execute(&mut observer)
        .await
        .unwrap();
    let blocked = sqlx::query("ALTER TABLE image_cancel ADD COLUMN must_stay_blocked INT")
        .execute(&mut observer)
        .await
        .unwrap_err();
    assert_eq!(
        blocked
            .as_database_error()
            .unwrap()
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .unwrap()
            .number(),
        1205
    );
    proxy.stall();
    let sql = SqlRecognizerFactory::recognize("DELETE FROM image_cancel WHERE id=?").unwrap();
    let params = [FieldValue::Signed(1)];
    let mut read = Box::pin(tx.before_image(&sql, &params));
    tokio::select! {outcome=&mut read=>panic!("read unexpectedly completed: {outcome:?}"),_ = tokio::time::timeout(Duration::from_secs(2),async{while proxy.held_bytes()==0{tokio::time::sleep(Duration::from_millis(5)).await}})=>{assert!(proxy.held_bytes()>0);}}
    println!("caller_cancel_held_bytes={}", proxy.held_bytes());
    drop(read);
    assert!(matches!(
        tx.table_meta("image_cancel").await,
        Err(DbError::Poisoned)
    ));
    sqlx::query("SET SESSION lock_wait_timeout=2")
        .execute(&mut observer)
        .await
        .unwrap();
    let released = sqlx::query("ALTER TABLE image_cancel ADD COLUMN after_cancel INT")
        .execute(&mut observer)
        .await;
    assert!(released.is_ok());
    proxy.shutdown().await;
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn response_loss_uses_one_client_deadline_for_the_whole_before_image_chain() {
    let port = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let proxy = stall_proxy::StallProxy::start(port).await;
    let ds = MySqlDataSource::connect(options(Some(proxy.port)))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    proxy.stall();
    let sql = SqlRecognizerFactory::recognize("DELETE FROM image_cancel WHERE id=?").unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(4),
        tx.before_image(&sql, &[FieldValue::Signed(1)]),
    )
    .await
    .expect("inner client deadline required");
    assert!(matches!(
        result,
        Err(DbError::Deadline {
            operation: "before image"
        })
    ));
    assert!(proxy.held_bytes() > 0);
    println!("image_deadline_held_bytes={}", proxy.held_bytes());
    proxy.shutdown().await;
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn insert_and_primary_key_update_are_rejected_without_business_execution() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let insert = SqlRecognizerFactory::recognize("INSERT INTO account(a,b) VALUES(?,?)").unwrap();
    assert!(matches!(
        tx.before_image(&insert, &[FieldValue::Signed(1), FieldValue::Signed(2)])
            .await,
        Err(DbError::Unsupported { .. })
    ));
    tx.table_meta("account").await.unwrap();
    let update = SqlRecognizerFactory::recognize("UPDATE account SET a=? WHERE b=?").unwrap();
    assert!(matches!(
        tx.before_image(
            &update,
            &[FieldValue::Signed(99), FieldValue::Signed(i64::MIN)]
        )
        .await,
        Err(DbError::Unsupported { .. })
    ));
    assert!(matches!(
        tx.table_meta("account").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let mut observer = admin().await;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM account WHERE a IN (1,2)")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(count, 2);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn composite_primary_order_is_database_index_order_not_physical_column_order() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize("DELETE FROM image_composite_order").unwrap();
    let result = tx.before_image(&sql, &[]).await;
    assert!(result.is_ok());
    let records = result.unwrap();
    let values: Vec<_> = records
        .rows()
        .iter()
        .map(|row| {
            (
                row.field("a").unwrap().value().clone(),
                row.field("b").unwrap().value().clone(),
            )
        })
        .collect();
    assert_eq!(
        values,
        vec![
            (FieldValue::Signed(1), FieldValue::Signed(9)),
            (FieldValue::Signed(2), FieldValue::Signed(0)),
            (FieldValue::Signed(2), FieldValue::Signed(1))
        ]
    );
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}
