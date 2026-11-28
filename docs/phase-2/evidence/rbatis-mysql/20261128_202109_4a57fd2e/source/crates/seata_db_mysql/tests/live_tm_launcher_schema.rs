//! TM 持久来源的真实 MySQL 结构门禁；这张表本身不授予跨进程 Launcher 权限。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/owned_mysql.rs"]
#[allow(dead_code)]
mod owned_mysql;

use seata_db_mysql::{DbError, MySqlDataSource, MySqlLauncherStore};
use seata_tm::Xid;
use sha2::{Digest, Sha256};
use sqlx::MySqlConnection;

async fn insert_source(
    admin: &mut MySqlConnection,
    digest: &[u8; 32],
    xid: &[u8],
    operation: u8,
    send_started: u8,
) -> Result<sqlx::mysql::MySqlQueryResult, sqlx::Error> {
    sqlx::query("INSERT INTO seata_rs_tm_launcher_v1 (schema_version,xid_digest,xid,tc_endpoint,application_id,service_group,begin_exception_code,business_intent,owner_instance_id,deadline_at,end_operation,send_started,created_at,modified_at) VALUES (1,?,?,?, ?,?,0,?,?,DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 60 SECOND),?,?,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
        .bind(digest.as_slice())
        .bind(xid)
        .bind(b"127.0.0.1:8091".as_slice())
        .bind(b"tm-app".as_slice())
        .bind(b"tm-group".as_slice())
        .bind(b"order-123".as_slice())
        .bind([1_u8; 32].as_slice())
        .bind(operation)
        .bind(send_started)
        .execute(admin)
        .await
}

fn assert_mysql_error(error: sqlx::Error, expected: u16) {
    assert_eq!(
        error
            .as_database_error()
            .and_then(|value| value.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>())
            .map(sqlx::mysql::MySqlDatabaseError::number),
        Some(expected)
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn legacy_008_row_remains_unowned_after_009_migration() {
    let mut admin = owned_mysql::admin().await;
    sqlx::raw_sql(include_str!(
        "../migrations/008_seata_rs_tm_launcher_v1.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let xid = Xid::parse("tc:legacy".into()).unwrap();
    let digest = Sha256::digest(xid.as_str().as_bytes());
    sqlx::query("INSERT INTO seata_rs_tm_launcher_v1 (schema_version,xid_digest,xid,tc_endpoint,application_id,service_group,begin_exception_code,business_intent,deadline_at,created_at,modified_at) VALUES (1,?,?,?,?,?,0,?,DATE_ADD(UTC_TIMESTAMP(6),INTERVAL 60 SECOND),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))")
        .bind(digest.as_slice()).bind(xid.as_str().as_bytes())
        .bind(b"127.0.0.1:8091".as_slice()).bind(b"tm-app".as_slice())
        .bind(b"tm-group".as_slice()).bind(b"order-legacy".as_slice())
        .execute(&mut admin).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/009_seata_rs_tm_launcher_owner_instance.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let bound_xid = Xid::parse("tc:instance-before-stop-scope".into()).unwrap();
    let bound_digest = Sha256::digest(bound_xid.as_str().as_bytes()).into();
    insert_source(
        &mut admin,
        &bound_digest,
        bound_xid.as_str().as_bytes(),
        0,
        0,
    )
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/010_seata_rs_tm_launcher_stop_scope.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let unbound: i64 = sqlx::query_scalar(
        "SELECT owner_instance_id IS NULL FROM seata_rs_tm_launcher_v1 WHERE xid=?",
    )
    .bind(xid.as_str().as_bytes())
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(unbound, 1);
    let changed = sqlx::query("UPDATE seata_rs_tm_launcher_v1 SET end_operation=1 WHERE xid_digest=? AND xid=? AND owner_instance_id=? AND owner_epoch=1 AND end_operation=0")
        .bind(digest.as_slice()).bind(xid.as_str().as_bytes())
        .bind([1_u8;32].as_slice()).execute(&mut admin).await.unwrap();
    assert_eq!(changed.rows_affected(), 0);
    let data_source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let store = MySqlLauncherStore::new(data_source.clone());
    assert!(matches!(
        store.load_snapshot(&xid).await,
        Err(DbError::RecoveryConflict)
    ));
    let legacy_bound = store.load_snapshot(&bound_xid).await.unwrap().unwrap();
    assert_eq!(legacy_bound.owner_instance_id(), &[1; 32]);
    assert_eq!(legacy_bound.owner_stop_scope(), None);
    data_source.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL 8.4.13 and a fresh undo_* database"]
async fn launcher_source_schema_preserves_full_xid_and_rejects_invalid_claim_states() {
    let mut admin = owned_mysql::admin().await;
    let db: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert!(db.starts_with("undo_") && db == std::env::var("SEATA_MYSQL_DATABASE").unwrap());
    sqlx::raw_sql(include_str!(
        "../migrations/008_seata_rs_tm_launcher_v1.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/009_seata_rs_tm_launcher_owner_instance.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();

    let longest_xid = vec![b'x'; i16::MAX as usize];
    insert_source(&mut admin, &[1_u8; 32], &longest_xid, 0, 0)
        .await
        .unwrap();
    let stored_length: i64 =
        sqlx::query_scalar("SELECT OCTET_LENGTH(xid) FROM seata_rs_tm_launcher_v1")
            .fetch_one(&mut admin)
            .await
            .unwrap();
    assert_eq!(stored_length, i16::MAX as i64);

    // 摘要碰撞或伪造只能被唯一索引拒绝；应用层仍必须比较完整 XID。
    assert_mysql_error(
        insert_source(&mut admin, &[1_u8; 32], b"different-xid", 0, 0)
            .await
            .unwrap_err(),
        1062,
    );
    assert_mysql_error(
        insert_source(&mut admin, &[2_u8; 32], b"", 0, 0)
            .await
            .unwrap_err(),
        3819,
    );
    assert_mysql_error(
        insert_source(&mut admin, &[3_u8; 32], b"end-without-intent", 0, 1)
            .await
            .unwrap_err(),
        3819,
    );
    assert_mysql_error(
        insert_source(&mut admin, &[4_u8; 32], b"unknown-operation", 3, 0)
            .await
            .unwrap_err(),
        3819,
    );
    // 独立连接同时竞争原意图和发送门闩；CAS 影响行数各只能出现一次 1。
    let mut first = owned_mysql::admin().await;
    let mut second = owned_mysql::admin().await;
    let intent_sql = "UPDATE seata_rs_tm_launcher_v1 SET end_operation=1,modified_at=UTC_TIMESTAMP(6) WHERE xid_digest=? AND xid=? AND owner_epoch=1 AND end_operation=0 AND send_started=0";
    let (a, b) = tokio::join!(
        sqlx::query(intent_sql)
            .bind([1_u8; 32].as_slice())
            .bind(longest_xid.as_slice())
            .execute(&mut first),
        sqlx::query(intent_sql)
            .bind([1_u8; 32].as_slice())
            .bind(longest_xid.as_slice())
            .execute(&mut second),
    );
    let mut affected = [a.unwrap().rows_affected(), b.unwrap().rows_affected()];
    affected.sort_unstable();
    assert_eq!(affected, [0, 1]);
    let send_sql = "UPDATE seata_rs_tm_launcher_v1 SET send_started=1,modified_at=UTC_TIMESTAMP(6) WHERE xid_digest=? AND xid=? AND owner_epoch=1 AND end_operation=1 AND send_started=0";
    let (a, b) = tokio::join!(
        sqlx::query(send_sql)
            .bind([1_u8; 32].as_slice())
            .bind(longest_xid.as_slice())
            .execute(&mut first),
        sqlx::query(send_sql)
            .bind([1_u8; 32].as_slice())
            .bind(longest_xid.as_slice())
            .execute(&mut second),
    );
    let mut affected = [a.unwrap().rows_affected(), b.unwrap().rows_affected()];
    affected.sort_unstable();
    assert_eq!(affected, [0, 1]);
    let state: (u8, u8, u64) = sqlx::query_as(
        "SELECT end_operation,send_started,owner_epoch FROM seata_rs_tm_launcher_v1 WHERE xid_digest=?",
    )
    .bind([1_u8; 32].as_slice())
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(state, (1, 1, 1));
    println!("TM launcher source shape and claim-state constraints verified in {db}");
}
