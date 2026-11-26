//! 同一真实事务的四条准备日志编码，输出交给原Java JDBC/parser独立对照；无TC登记或提交。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_at::{
    BaseTransactionalExecutor, ConnectionContext, JacksonUndoLogParser, MySqlJdbcProfile,
    SqlRecognizerFactory, SqlType,
};
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
    if let Ok(directory) = std::env::var("SEATA_MYSQL_LOCK_KEYS_OUT") {
        let directory = PathBuf::from(directory);
        assert!(directory.is_absolute() && directory.to_string_lossy().starts_with("D:"));
        fs::create_dir_all(&directory).unwrap();
        for sql in ["INSERT INTO lock_pk_reverse(a,b) VALUES(1,2)","INSERT INTO lock_tiny_keys(id) VALUES(0),(1)",
            "INSERT INTO lock_decimal_keys(id) VALUES(0.00000000000000000000),(0.00000010000000000000)"] {
            tx.execute(&SqlRecognizerFactory::recognize(sql).unwrap(),&[]).await.unwrap();
        }
        tx.execute(
            &SqlRecognizerFactory::recognize("INSERT INTO lock_binary_keys(id) VALUES(?),(?)")
                .unwrap(),
            &[
                FieldValue::Binary(vec![0, 255]),
                FieldValue::Binary(vec![128, 127]),
            ],
        )
        .await
        .unwrap();
        tx.execute(
            &SqlRecognizerFactory::recognize(include_str!(
                "../../../tools/formal-lock-key/collision-insert.sql"
            ))
            .unwrap(),
            &[],
        )
        .await
        .unwrap();
        assert_eq!(tx.undo_logs().len(), 9);
        for (name, profile) in [
            ("default", MySqlJdbcProfile::Connector8Default),
            (
                "numeric_tinyint",
                MySqlJdbcProfile::Connector8NumericTinyInt,
            ),
        ] {
            let keys: Vec<_> = tx
                .undo_logs()
                .iter()
                .map(|log| {
                    BaseTransactionalExecutor::build_lock_key(
                        if log.sql_type() == SqlType::Delete {
                            log.before_image()
                        } else {
                            log.after_image()
                        },
                        profile,
                    )
                    .unwrap()
                    .unwrap()
                })
                .collect();
            let context = ConnectionContext::from_undo_logs(tx.undo_logs(), profile).unwrap();
            let full: Vec<_> = context
                .build_lock_keys()
                .unwrap()
                .split(';')
                .map(str::to_owned)
                .collect();
            assert_eq!(keys[4], "lock_pk_reverse:1_2");
            assert_eq!(
                keys[5],
                if name == "default" {
                    "lock_tiny_keys:false,true"
                } else {
                    "lock_tiny_keys:0,1"
                }
            );
            assert_eq!(keys[6], "lock_decimal_keys:0E-20,1.0000000000000E-7");
            assert_eq!(keys[7], "lock_binary_keys:[0, -1],[-128, 127]");
            fs::write(
                directory.join(format!("{name}.json")),
                serde_json::to_vec(&serde_json::json!({"keys":keys,"full_key_set":full})).unwrap(),
            )
            .unwrap();
        }
        println!("actual_lock_logs=9 profiles=2 registration=false");
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
    if std::env::var_os("SEATA_MYSQL_LOCK_KEYS_OUT").is_some() {
        for table in [
            "lock_pk_reverse",
            "lock_tiny_keys",
            "lock_decimal_keys",
            "lock_binary_keys",
            "lock_hash_collision",
        ] {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM `{table}`"))
                .fetch_one(&mut observer)
                .await
                .unwrap();
            assert_eq!(count, 0);
        }
    }
    println!("actual_prepared_logs=4 profiles=2 rollback=true public_commit=false");
}
