//! 自有Undo存储夹具；原表DDL由运行器从固定Git blob提供，不作用于共享数据库。
use super::owned_mysql;
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::{DbError, MySqlDataSource, MySqlLocalTransaction};
use sqlx::{MySqlConnection, Row};
use std::{collections::BTreeMap, future::Future, path::PathBuf, pin::Pin, time::Duration};

/// 无参数；在运行器自有库恢复原Undo DDL及空业务表，返回测试管理连接。
pub async fn reset() -> MySqlConnection {
    let mut admin = owned_mysql::admin().await;
    let db: String = sqlx::query_scalar("SELECT DATABASE()")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert!(db.starts_with("undo_") && db == std::env::var("SEATA_MYSQL_DATABASE").unwrap());
    for sql in [
        "DROP TABLE IF EXISTS undo_child",
        "DROP TABLE IF EXISTS undo_work",
        "DROP TABLE IF EXISTS undo_business",
        "DROP TABLE IF EXISTS undo_log",
    ] {
        sqlx::query(sql).execute(&mut admin).await.unwrap();
    }
    let path = PathBuf::from(std::env::var("SEATA_UNDO_SCHEMA_PATH").unwrap());
    assert!(path.is_absolute() && path.to_string_lossy().starts_with("D:"));
    sqlx::raw_sql(&std::fs::read_to_string(path).unwrap())
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL max_allowed_packet=67108864")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE undo_work(id INT PRIMARY KEY,v VARCHAR(1000)) ENGINE=InnoDB")
        .execute(&mut admin)
        .await
        .unwrap();
    admin
}
/// 参数为可选操作期限；返回已准备一次真实业务写入的源与未提交事务。
pub async fn prepared(timeout: Option<Duration>) -> (MySqlDataSource, MySqlLocalTransaction) {
    let options = owned_mysql::options(None);
    let ds = MySqlDataSource::connect(if let Some(value) = timeout {
        options.operation_timeout(value).unwrap()
    } else {
        options
    })
    .await
    .unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("INSERT INTO undo_work(id,v) VALUES(1,'once')").unwrap(),
        &[],
    )
    .await
    .unwrap();
    (ds, tx)
}
/// 参数为管理连接与测试常量表名；返回实际可见行数。
pub async fn count(admin: &mut MySqlConnection, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(admin)
        .await
        .unwrap()
}
/// 参数为独立观察者；切换为脏读以观察尚未提交的实际行。
pub async fn dirty(admin: &mut MySqlConnection) {
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL READ UNCOMMITTED")
        .execute(admin)
        .await
        .unwrap();
}
/// 参数为实际原context；返回测试用键值视图，不执行任何恢复行为。
pub fn context(value: &str) -> BTreeMap<&str, &str> {
    value
        .split('&')
        .map(|part| part.split_once('=').unwrap())
        .collect()
}
/// 参数为证据文件名与采样值；仅写入运行器生成的D盘目录。
pub fn save(name: &str, value: &serde_json::Value) {
    let folder = PathBuf::from(std::env::var("SEATA_UNDO_STORE_OUT").unwrap());
    assert!(folder.is_absolute() && folder.to_string_lossy().starts_with("D:"));
    std::fs::write(folder.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
/// 参数为观察者、文件名与预期状态；核对每行原身份/状态/时间并保存全部七列。
pub async fn snapshot(
    admin: &mut MySqlConnection,
    name: &str,
    expected_status: i32,
) -> Vec<(i64, String, Vec<u8>)> {
    let rows = sqlx::query(
        "SELECT branch_id,xid,context,rollback_info,log_status,CAST(log_created AS CHAR CHARACTER SET utf8mb4) AS log_created,CAST(log_modified AS CHAR CHARACTER SET utf8mb4) AS log_modified FROM undo_log ORDER BY branch_id",
    )
    .fetch_all(admin)
    .await
    .unwrap();
    for row in &rows {
        assert_eq!(
            row.get::<String, _>("xid"),
            "undo-store:xid",
            "every original root/sub-row must retain its XID"
        );
        assert_eq!(
            row.get::<i32, _>("log_status"),
            expected_status,
            "every original row must retain its Normal/defense state"
        );
        assert_eq!(
            row.get::<String, _>("log_created"),
            row.get::<String, _>("log_modified"),
            "original insert timestamps must agree"
        );
    }
    let values: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r.get::<i64, _>("branch_id"),
                r.get::<String, _>("context"),
                r.get::<Vec<u8>, _>("rollback_info"),
            )
        })
        .collect();
    save(
        name,
        &serde_json::json!({"rows":rows.iter().map(|r|serde_json::json!({"branch_id":r.get::<i64,_>("branch_id"),"xid":r.get::<String,_>("xid"),"context":r.get::<String,_>("context"),"bytes":r.get::<Vec<u8>,_>("rollback_info"),"status":r.get::<i32,_>("log_status"),"log_created":r.get::<String,_>("log_created"),"log_modified":r.get::<String,_>("log_modified")})).collect::<Vec<_>>()}),
    );
    values
}
/// 参数为在途flush、独立观察者与证据名；实际看到Undo唯一键等待才返回。
pub async fn wait_blocked<F: Future<Output = Result<(), DbError>>>(
    future: &mut Pin<Box<F>>,
    admin: &mut MySqlConnection,
    name: &str,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    loop {
        tokio::select! { result=future.as_mut()=>panic!("flush ended before a real unique-key wait: {result:?}"),_ = tokio::time::sleep(Duration::from_millis(5))=>{} }
        let waits:i64=sqlx::query_scalar("SELECT COUNT(*) FROM performance_schema.data_lock_waits AS w JOIN performance_schema.data_locks AS l ON w.REQUESTING_ENGINE_LOCK_ID=l.ENGINE_LOCK_ID WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='undo_log'").fetch_one(&mut *admin).await.unwrap();
        if waits > 0 {
            save(
                name,
                &serde_json::json!({"waits":waits,"xid":"undo-store:xid","branch_id":42}),
            );
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "original Undo insert did not reach real key arbitration"
        );
    }
}
