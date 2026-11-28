//! 本地DML及内存Undo准备；独立连接验证没有绕过Undo的隐式提交。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_at::{SqlRecognizerFactory, SqlType};
use seata_datasource::{DecimalValue, FieldValue, ImageError};
use seata_db_mysql::{DbError, MySqlDataSource};
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
use owned_mysql::{admin, options};
#[path = "support/execution_failures.rs"]
mod execution_failures;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn insert_uses_explicit_composite_keys_and_native_exact_values_without_committing() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let mut observer = admin().await;
    sqlx::query("SET GLOBAL log_output='TABLE'")
        .execute(&mut observer)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=ON")
        .execute(&mut observer)
        .await
        .unwrap();
    let sql=SqlRecognizerFactory::recognize("INSERT INTO execution_records(note,a,b,amount,payload) VALUES ('literal ?', ?, ? - 1, ?, ?), (?,7,?,?,NULL)").unwrap();
    let huge =
        DecimalValue::new("12345678901234567890123456789012345.123456789012345678901234567890")
            .unwrap();
    let tiny = DecimalValue::new("-0.000000000000000000000000000001").unwrap();
    let parameters = [
        FieldValue::Unsigned(u64::MAX),
        FieldValue::Signed(i64::MIN + 1),
        FieldValue::Decimal(huge.clone()),
        FieldValue::Binary(vec![0, 255]),
        FieldValue::Text("second".into()),
        FieldValue::Signed(2),
        FieldValue::Decimal(tiny.clone()),
    ];
    assert_eq!(tx.execute(&sql, &parameters).await.unwrap(), 2);
    assert_eq!(tx.undo_logs().len(), 1);
    let undo = &tx.undo_logs()[0];
    assert_eq!(undo.sql_type(), SqlType::Insert);
    assert!(undo.before_image().rows().is_empty());
    let rows = undo.after_image().rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].field("a").unwrap().value(),
        &FieldValue::Unsigned(7)
    );
    assert_eq!(
        rows[0].field("amount").unwrap().value(),
        &FieldValue::Decimal(tiny)
    );
    assert_eq!(
        rows[1].field("a").unwrap().value(),
        &FieldValue::Unsigned(u64::MAX)
    );
    assert_eq!(
        rows[1].field("b").unwrap().value(),
        &FieldValue::Signed(i64::MIN)
    );
    assert_eq!(
        rows[1].field("amount").unwrap().value(),
        &FieldValue::Decimal(huge)
    );
    assert_eq!(
        rows[1].field("payload").unwrap().value(),
        &FieldValue::Binary(vec![0, 255])
    );
    assert_eq!(
        rows[1].field("note").unwrap().value(),
        &FieldValue::Text("literal ?".into())
    );
    let visible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_records")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(visible, 0);
    let native:i64=sqlx::query_scalar("SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Execute' AND argument LIKE 'INSERT INTO execution_records%'").fetch_one(&mut observer).await.unwrap();
    assert_eq!(native, 1, "original business SQL executes once");
    let durable: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM undo_log")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(durable, 0);
    tx.rollback().await.unwrap();
    let visible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_records")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(visible, 0);
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut observer)
        .await
        .unwrap();
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn update_after_image_uses_original_keys_when_the_where_column_changes_then_delete_is_empty()
{
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let update =
        SqlRecognizerFactory::recognize("UPDATE execution_work SET v=v+?,note=? WHERE v=?")
            .unwrap();
    let result = tx
        .execute(
            &update,
            &[
                FieldValue::Signed(5),
                FieldValue::Text("changed".into()),
                FieldValue::Signed(10),
            ],
        )
        .await;
    assert!(
        result.is_ok(),
        "after image must follow captured primary keys: {result:?}"
    );
    assert_eq!(result.unwrap(), 1);
    let undo = &tx.undo_logs()[0];
    assert_eq!(
        undo.before_image().rows()[0].field("v").unwrap().value(),
        &FieldValue::Signed(10)
    );
    assert_eq!(
        undo.after_image().rows()[0].field("v").unwrap().value(),
        &FieldValue::Signed(15)
    );
    let delete =
        SqlRecognizerFactory::recognize("DELETE FROM execution_work WHERE note=?").unwrap();
    assert_eq!(
        tx.execute(&delete, &[FieldValue::Text("changed".into())])
            .await
            .unwrap(),
        1
    );
    assert_eq!(tx.undo_logs().len(), 2);
    let deletion = &tx.undo_logs()[1];
    assert_eq!(deletion.sql_type(), SqlType::Delete);
    assert_eq!(deletion.before_image().rows().len(), 1);
    assert!(deletion.after_image().rows().is_empty());
    let mut observer = admin().await;
    let visible: i32 = sqlx::query_scalar("SELECT v FROM execution_work WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(visible, 10);
    tx.rollback().await.unwrap();
    let visible: i32 = sqlx::query_scalar("SELECT v FROM execution_work WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(visible, 10);
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn no_match_and_noop_do_not_invent_rows_or_require_a_changed_row_count() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let noop = SqlRecognizerFactory::recognize("UPDATE execution_work SET v=v WHERE id=1").unwrap();
    let changed = tx.execute(&noop, &[]).await.unwrap();
    assert!(changed <= 1);
    assert_eq!(tx.undo_logs().len(), 1);
    assert_eq!(
        tx.undo_logs()[0].before_image(),
        tx.undo_logs()[0].after_image()
    );
    for text in [
        "UPDATE execution_work SET v=9 WHERE id=999",
        "DELETE FROM execution_work WHERE id=999",
    ] {
        let sql = SqlRecognizerFactory::recognize(text).unwrap();
        assert_eq!(tx.execute(&sql, &[]).await.unwrap(), 0);
    }
    assert_eq!(tx.undo_logs().len(), 1);
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn missing_explicit_insert_key_and_primary_key_update_fail_before_business_execution() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    for text in [
        "INSERT INTO execution_records(b,amount) VALUES(1,1)",
        "UPDATE execution_work SET id=3 WHERE id=1",
    ] {
        let mut tx = ds.begin().await.unwrap();
        let sql = SqlRecognizerFactory::recognize(text).unwrap();
        assert!(matches!(
            tx.execute(&sql, &[]).await,
            Err(DbError::Unsupported { .. })
        ));
        assert!(tx.undo_logs().is_empty());
        assert!(matches!(
            tx.execute(&sql, &[]).await,
            Err(DbError::Poisoned)
        ));
        tx.rollback().await.unwrap();
    }
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn duplicate_key_failure_poisons_and_rollback_removes_all_prior_local_effects() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let good =
        SqlRecognizerFactory::recognize("UPDATE execution_work SET v=11 WHERE id=1").unwrap();
    tx.execute(&good, &[]).await.unwrap();
    let bad =
        SqlRecognizerFactory::recognize("INSERT INTO execution_work(id,v) VALUES(1,99)").unwrap();
    assert!(matches!(
        tx.execute(&bad, &[]).await,
        Err(DbError::Driver {
            code: Some(1062),
            ..
        })
    ));
    assert_eq!(tx.undo_logs().len(), 1);
    assert!(matches!(
        tx.execute(&good, &[]).await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let mut observer = admin().await;
    let visible: i32 = sqlx::query_scalar("SELECT v FROM execution_work WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(visible, 10);
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn after_image_over_budget_is_not_published_and_the_business_effects_roll_back() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize("UPDATE execution_budget SET payload=?").unwrap();
    let result = tx
        .execute(&sql, &[FieldValue::Binary(vec![b'x'; 1_000_000])])
        .await;
    assert!(matches!(
        result,
        Err(DbError::Image(ImageError::Budget { .. }))
    ));
    assert!(tx.undo_logs().is_empty());
    assert!(matches!(
        tx.table_meta("execution_budget").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let mut observer = admin().await;
    let bytes: Option<bigdecimal::BigDecimal> =
        sqlx::query_scalar("SELECT SUM(OCTET_LENGTH(payload)) FROM execution_budget")
            .fetch_one(&mut observer)
            .await
            .unwrap();
    assert_eq!(bytes.unwrap().to_plain_string(), "9");
    ds.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn local_parameter_rejection_preserves_the_scope_without_appending_undo() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize("UPDATE execution_work SET v=? WHERE id=1").unwrap();
    assert!(matches!(
        tx.execute(&sql, &[]).await,
        Err(DbError::Input { .. })
    ));
    assert!(tx.undo_logs().is_empty());
    tx.table_meta("execution_work").await.unwrap();
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn decimal_text_primary_assignment_cannot_capture_an_adjacent_old_row() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql =
        SqlRecognizerFactory::recognize("INSERT INTO execution_rounding(id,note) VALUES('1.6',?)")
            .unwrap();
    assert_eq!(
        tx.execute(&sql, &[FieldValue::Text("new-two".into())])
            .await
            .unwrap(),
        1
    );
    let image = tx.undo_logs()[0].after_image();
    assert_eq!(image.rows().len(), 1);
    println!(
        "adjacent_old_row_captured={}",
        image.rows()[0].field("id").unwrap().value() == &FieldValue::Signed(1)
            && image.rows()[0].field("note").unwrap().value()
                == &FieldValue::Text("old-one".into())
    );
    assert_eq!(
        image.rows()[0].field("id").unwrap().value(),
        &FieldValue::Signed(2)
    );
    assert_eq!(
        image.rows()[0].field("note").unwrap().value(),
        &FieldValue::Text("new-two".into())
    );
    tx.rollback().await.unwrap();
    let mut observer = admin().await;
    let old: String = sqlx::query_scalar("SELECT note FROM execution_rounding WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(old, "old-one");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_rounding WHERE id=2")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(count, 0);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn pending_undo_record_limit_poisons_and_forces_full_rollback() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql =
        SqlRecognizerFactory::recognize("UPDATE execution_work SET v=v+1 WHERE id=1").unwrap();
    for _ in 0..128 {
        tx.execute(&sql, &[]).await.unwrap();
    }
    assert_eq!(tx.undo_logs().len(), 128);
    assert!(matches!(
        tx.execute(&sql, &[]).await,
        Err(DbError::Image(ImageError::Budget { .. }))
    ));
    assert!(matches!(
        tx.table_meta("execution_work").await,
        Err(DbError::Poisoned)
    ));
    assert!(matches!(
        tx.execute(&sql, &[]).await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let mut observer = admin().await;
    let value: i32 = sqlx::query_scalar("SELECT v FROM execution_work WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(value, 10);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn key_assignment_staging_preserves_char_binary_decimal_and_does_not_commit_prior_work() {
    let ds = MySqlDataSource::connect(options(None)).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let mut observer = admin().await;
    let update =
        SqlRecognizerFactory::recognize("UPDATE execution_work SET v=11 WHERE id=1").unwrap();
    tx.execute(&update, &[]).await.unwrap();
    let insert = SqlRecognizerFactory::recognize(
        "INSERT INTO execution_mixed_keys(tc,bk,dk,note) VALUES(?,?,?,?)",
    )
    .unwrap();
    let args = [
        FieldValue::Text("x ".into()),
        FieldValue::Binary(vec![255]),
        FieldValue::Decimal(DecimalValue::new("1.235").unwrap()),
        FieldValue::Text("typed-staging".into()),
    ];
    assert_eq!(tx.execute(&insert, &args).await.unwrap(), 1);
    assert_eq!(tx.undo_logs().len(), 2);
    let row = &tx.undo_logs()[1].after_image().rows()[0];
    assert_eq!(
        row.field("tc").unwrap().value(),
        &FieldValue::Text("x".into())
    );
    assert_eq!(
        row.field("bk").unwrap().value(),
        &FieldValue::Binary(vec![255, 0, 0, 0])
    );
    assert_eq!(
        row.field("dk").unwrap().value(),
        &FieldValue::Decimal(DecimalValue::new("1.24").unwrap())
    );
    let old: i32 = sqlx::query_scalar("SELECT v FROM execution_work WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(
        old, 10,
        "temporary DDL must not implicitly commit previous business"
    );
    let visible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_mixed_keys")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(visible, 0);
    let temporary: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.INNODB_TEMP_TABLE_INFO")
            .fetch_one(&mut observer)
            .await
            .unwrap();
    assert_eq!(
        temporary, 0,
        "successful key staging must drop the temporary table"
    );
    tx.rollback().await.unwrap();
    let old: i32 = sqlx::query_scalar("SELECT v FROM execution_work WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(old, 10);
    ds.close().await.unwrap();
}
