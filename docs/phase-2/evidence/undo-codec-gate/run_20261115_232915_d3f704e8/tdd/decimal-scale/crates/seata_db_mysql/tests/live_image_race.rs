//! 在完整值查询送往服务端之前，独立证明行锁和间隙锁防止两查询间的并发变化。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_at::SqlRecognizerFactory;
use seata_datasource::FieldValue;
use seata_db_mysql::MySqlDataSource;
use std::time::Duration;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/query_barrier_proxy.rs"]
mod query_barrier_proxy;
use owned_mysql::{admin, options};

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn preflight_locks_prevent_row_growth_and_phantoms_before_full_query_is_forwarded() {
    let port = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let proxy = query_barrier_proxy::QueryBarrierProxy::start(
        port,
        b"\x16SELECT `id`, `payload` FROM `image_inter_query`".to_vec(),
    )
    .await;
    let ds = MySqlDataSource::connect(
        options(Some(proxy.port))
            .operation_timeout(Duration::from_secs(10))
            .unwrap(),
    )
    .await
    .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let sql = SqlRecognizerFactory::recognize("DELETE FROM image_inter_query WHERE id>=?").unwrap();
    let args = [FieldValue::Signed(1)];
    let mut read = Box::pin(tx.before_image(&sql, &args));
    tokio::select! {result=&mut read=>panic!("full value read completed before barrier: {result:?}"),barrier=tokio::time::timeout(Duration::from_secs(4),async{while !proxy.captured(){tokio::time::sleep(Duration::from_millis(5)).await}})=>{assert!(barrier.is_ok());assert!(proxy.captured());}}
    let raw = proxy
        .captured_packet()
        .expect("actual original frame retained");
    assert!(raw.len() > 5);
    let declared = u32::from_le_bytes([raw[0], raw[1], raw[2], 0]) as usize;
    assert_eq!(declared, raw.len() - 4);
    assert_eq!(raw[3], 0);
    assert!(raw[4..].starts_with(b"\x16SELECT `id`, `payload` FROM `image_inter_query`"));
    println!(
        "query_barrier_packet_hex={}",
        raw.iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let mut observer = admin().await;
    sqlx::query("SET SESSION innodb_lock_wait_timeout=1")
        .execute(&mut observer)
        .await
        .unwrap();
    let growth = tokio::select! {result=&mut read=>panic!("reader escaped growth barrier: {result:?}"),result=sqlx::query("UPDATE image_inter_query SET payload=REPEAT('b',1048577) WHERE id=1").execute(&mut observer)=>result};
    assert!(
        growth.is_err(),
        "row growth must be blocked before full value query"
    );
    assert_eq!(
        growth
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .unwrap()
            .number(),
        1205
    );
    let insert = tokio::select! {result=&mut read=>panic!("reader escaped insert barrier: {result:?}"),result=sqlx::query("INSERT INTO image_inter_query VALUES(2,X'79')").execute(&mut observer)=>result};
    assert!(
        insert.is_err(),
        "phantom insert must be blocked before full value query"
    );
    assert_eq!(
        insert
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .unwrap()
            .number(),
        1205
    );
    proxy.release();
    let image = read.await;
    assert!(image.is_ok());
    let image = image.unwrap();
    assert_eq!(image.rows().len(), 1);
    assert_eq!(
        image.rows()[0].field("payload").unwrap().value(),
        &FieldValue::Binary(vec![b'x'])
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        sqlx::query("UPDATE image_inter_query SET payload=REPEAT('b',1048577) WHERE id=1")
            .execute(&mut observer)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    assert_eq!(
        sqlx::query("INSERT INTO image_inter_query VALUES(2,X'79')")
            .execute(&mut observer)
            .await
            .unwrap()
            .rows_affected(),
        1
    );
    println!("inter_query_barrier=true growth_blocked=1205 insert_blocked=1205 image_rows=1 payload_bytes=1 after_rollback_growth=1 after_rollback_insert=1");
    proxy.shutdown().await;
    ds.close().await.unwrap();
}
