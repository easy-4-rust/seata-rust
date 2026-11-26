//! 同一真实事务的四条准备日志编码，输出交给原Java JDBC/parser独立对照；无TC登记或提交。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_at::{JacksonUndoLogParser, MySqlJdbcProfile, SqlRecognizerFactory};
use seata_datasource::{DecimalValue, FieldValue};
use seata_db_mysql::MySqlDataSource;
use std::{fs, path::PathBuf};
#[path = "support/owned_mysql.rs"]
mod owned_mysql;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn real_prepared_insert_update_delete_and_exact_composite_rows_encode_without_commit() {
    let out = PathBuf::from(std::env::var("SEATA_MYSQL_CODEC_OUT").unwrap());
    assert!(out.is_absolute() && out.to_string_lossy().starts_with("D:"));
    fs::create_dir_all(&out).unwrap();
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let insert = SqlRecognizerFactory::recognize(include_str!(
        "../../../tests/fixtures/undo/business_sql/native_insert.sql"
    ))
    .unwrap();
    let args = [
        vec![255],
        vec![0, 255],
        vec![0, 255],
        vec![128, 255],
        vec![0, 255, 127, 128],
        vec![0, 1, 255],
    ]
    .map(FieldValue::Binary);
    assert_eq!(tx.execute(&insert, &args).await.unwrap(), 1);
    for text in [
        include_str!("../../../tests/fixtures/undo/business_sql/native_update.sql"),
        include_str!("../../../tests/fixtures/undo/business_sql/native_delete.sql"),
    ] {
        let sql = SqlRecognizerFactory::recognize(text).unwrap();
        assert_eq!(tx.execute(&sql, &[]).await.unwrap(), 1);
    }
    let composite = SqlRecognizerFactory::recognize(include_str!(
        "../../../tests/fixtures/undo/business_sql/composite_insert.sql"
    ))
    .unwrap();
    let values = [
        FieldValue::Signed(i64::MIN),
        FieldValue::Unsigned(u64::MAX),
        FieldValue::Decimal(
            DecimalValue::new("12345678901234567890123456789012345.123456789012345678901234567890")
                .unwrap(),
        ),
        FieldValue::Text("中文🦀".into()),
        FieldValue::Binary(vec![0, 255]),
        FieldValue::Signed(2),
        FieldValue::Unsigned(7),
        FieldValue::Decimal(DecimalValue::new("-0.000000000000000000000000000001").unwrap()),
        FieldValue::Null,
        FieldValue::Null,
    ];
    assert_eq!(tx.execute(&composite, &values).await.unwrap(), 2);
    assert_eq!(tx.undo_logs().len(), 4);
    let codec = JacksonUndoLogParser::default();
    for (name, profile) in [
        ("default", MySqlJdbcProfile::Connector8Default),
        (
            "numeric_tinyint",
            MySqlJdbcProfile::Connector8NumericTinyInt,
        ),
    ] {
        let bytes = codec
            .encode_prepared("codec-fixture:xid", 42, tx.undo_logs(), profile)
            .unwrap();
        assert_eq!(
            codec
                .decode(&bytes)
                .unwrap()
                .unwrap()
                .sql_undo_logs()
                .unwrap()
                .len(),
            4
        );
        fs::write(out.join(format!("{name}.json")), bytes).unwrap();
    }
    let mut observer = owned_mysql::admin().await;
    let visible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_records")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(visible, 0);
    let undo: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM undo_log")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(undo, 0, "encoding does not persist or commit");
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    for table in ["execution_records", "mysql_native_identity"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM `{table}`"))
            .fetch_one(&mut observer)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    println!("actual_prepared_logs=4 profiles=2 rollback=true public_commit=false");
}
