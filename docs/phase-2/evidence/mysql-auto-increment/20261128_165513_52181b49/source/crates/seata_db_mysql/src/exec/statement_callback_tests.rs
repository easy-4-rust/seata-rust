//! 仅验证原 StatementCallback 的 OK 回执，不声称正式 AT 已准入自增表。
#![allow(clippy::unwrap_used)]
use super::StatementCallback;
use seata_at::SqlRecognizerFactory;
use seata_datasource::FieldValue;
use sqlx::Row;

#[tokio::test]
#[ignore = "requires owned MySQL; run native_seaorm_mysql.py"]
async fn original_ok_receipt_survives_later_queries_without_signed_truncation() {
    let options = sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap())
        .username("root")
        .password(&std::env::var("SEATA_MYSQL_PASSWORD").unwrap())
        .database(&std::env::var("SEATA_MYSQL_DATABASE").unwrap())
        .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled);
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("CREATE TEMPORARY TABLE receipt_items(id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB AUTO_INCREMENT=9223372036854775811")
        .execute(&mut *tx).await.unwrap();
    let insert =
        SqlRecognizerFactory::recognize("INSERT INTO receipt_items(v) VALUES (?)").unwrap();
    let first = StatementCallback::execute(&mut tx, &insert, &[FieldValue::Signed(10)])
        .await
        .unwrap();
    assert_eq!(
        (first.rows_affected(), first.last_insert_id()),
        (1, 9223372036854775811)
    );
    let actual: u64 = sqlx::query_scalar("SELECT id FROM receipt_items WHERE v=10")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(first.last_insert_id(), actual);
    let later: u64 = sqlx::query_scalar("SELECT LAST_INSERT_ID(7)")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(later, 7);
    assert_eq!(first.last_insert_id(), actual);
    let batch =
        SqlRecognizerFactory::recognize("INSERT INTO receipt_items(v) VALUES (?), (?)").unwrap();
    let batch = StatementCallback::execute(
        &mut tx,
        &batch,
        &[FieldValue::Signed(20), FieldValue::Signed(30)],
    )
    .await
    .unwrap();
    assert_eq!(batch.rows_affected(), 2);
    let ids: Vec<u64> =
        sqlx::query_scalar("SELECT id FROM receipt_items WHERE v IN (20,30) ORDER BY v")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(batch.last_insert_id(), ids[0]);
    // 对照同一连接的原协议结果：UPDATE/DELETE 的 OK ID 不可替换为会话 LAST_INSERT_ID。
    let update =
        SqlRecognizerFactory::recognize("UPDATE receipt_items SET v=? WHERE id=?").unwrap();
    let updated = StatementCallback::execute(
        &mut tx,
        &update,
        &[FieldValue::Signed(11), FieldValue::Unsigned(actual)],
    )
    .await
    .unwrap();
    assert_eq!((updated.rows_affected(), updated.last_insert_id()), (1, 0));
    let delete = SqlRecognizerFactory::recognize("DELETE FROM receipt_items WHERE id=?").unwrap();
    let deleted = StatementCallback::execute(&mut tx, &delete, &[FieldValue::Unsigned(actual)])
        .await
        .unwrap();
    assert_eq!((deleted.rows_affected(), deleted.last_insert_id()), (1, 0));
    let duplicate =
        SqlRecognizerFactory::recognize("INSERT INTO receipt_items(id,v) VALUES (?,?)").unwrap();
    assert!(StatementCallback::execute(
        &mut tx,
        &duplicate,
        &[FieldValue::Unsigned(ids[0]), FieldValue::Signed(99)]
    )
    .await
    .is_err());
    let observed = sqlx::query("SELECT id,v FROM receipt_items ORDER BY v")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    let rows: Vec<(u64, i32)> = observed.iter().map(|r| (r.get(0), r.get(1))).collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].1, 20);
    assert_eq!(rows[1].1, 30);
    tx.rollback().await.unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM receipt_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    let report = serde_json::json!({"scope":"original callback only; not AT auto increment admission", "first":{"rows_affected":first.rows_affected(),"last_insert_id":first.last_insert_id()}, "actual_first_id":actual,"later_session_id":later,
        "batch":{"rows_affected":batch.rows_affected(),"last_insert_id":batch.last_insert_id()},"actual_batch_ids":ids,"update_id":updated.last_insert_id(),"delete_id":deleted.last_insert_id(),"duplicate_rejected":true,"rows_before_rollback":rows,"rows_after_rollback":remaining});
    std::fs::write(
        std::env::var("SEATA_STATEMENT_RECEIPT_OUT").unwrap(),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
