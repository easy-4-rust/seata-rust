//! 重建原生未提交准备字节及其真实后镜像为已提交夹具；不向正式API添加原业务提交入口。
use super::{fixture, owned_mysql};
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::MySqlDataSource;
use sqlx::{Connection, MySqlConnection, Row};
use std::time::Duration;

pub async fn seed(
    initial: &[&str],
    statements: &[&str],
    profile: MySqlJdbcProfile,
    packet: Option<u64>,
) -> (MySqlConnection, MySqlDataSource) {
    let mut admin = fixture::reset().await;
    for sql in initial {
        sqlx::raw_sql(sql).execute(&mut admin).await.unwrap();
    }
    if let Some(packet) = packet {
        sqlx::query(&format!("SET GLOBAL max_allowed_packet={packet}"))
            .execute(&mut admin)
            .await
            .unwrap();
    }
    let ds = MySqlDataSource::connect(
        owned_mysql::options(None)
            .operation_timeout(Duration::from_secs(20))
            .unwrap(),
    )
    .await
    .unwrap();
    let mut tx = ds.begin().await.unwrap();
    for sql in statements {
        tx.execute(&SqlRecognizerFactory::recognize(sql).unwrap(), &[])
            .await
            .unwrap();
    }
    tx.flush_undo_logs("rollback:xid", 42, profile)
        .await
        .unwrap();
    fixture::dirty(&mut admin).await;
    let rows = snapshot(&mut admin).await;
    assert!(!rows.is_empty());
    fixture::save(
        "seed-native-rows.json",
        &serde_json::json!({"rows":rows,"initial":initial,"statements":statements}),
    );
    static CASE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fixture::save(
        &format!(
            "native-case-{}.json",
            CASE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ),
        &serde_json::json!({"rows":rows,"initial":initial,"statements":statements,"profile":if profile==MySqlJdbcProfile::Connector8Default{"default"}else{"numeric_tinyint"},"packet":packet}),
    );
    tx.rollback().await.unwrap();
    sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut admin)
        .await
        .unwrap();
    let mut seed = admin.begin().await.unwrap();
    for sql in statements {
        sqlx::raw_sql(sql).execute(&mut *seed).await.unwrap();
    }
    for row in &rows {
        let bytes: Vec<u8> = row["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| u8::try_from(n.as_u64().unwrap()).unwrap())
            .collect();
        sqlx::query(
            "INSERT INTO undo_log VALUES(?,?,?,?,?,CAST(? AS DATETIME(6)),CAST(? AS DATETIME(6)))",
        )
        .bind(row["branch_id"].as_i64().unwrap())
        .bind(row["xid"].as_str().unwrap())
        .bind(row["context"].as_str().unwrap())
        .bind(bytes)
        .bind(row["status"].as_i64().unwrap())
        .bind(row["created"].as_str().unwrap())
        .bind(row["modified"].as_str().unwrap())
        .execute(&mut *seed)
        .await
        .unwrap();
    }
    seed.commit().await.unwrap();
    assert_eq!(rows, snapshot(&mut admin).await);
    (admin, ds)
}
pub async fn snapshot(admin: &mut MySqlConnection) -> Vec<serde_json::Value> {
    sqlx::query("SELECT xid,branch_id,context,rollback_info,log_status,CAST(log_created AS CHAR) AS created,CAST(log_modified AS CHAR) AS modified FROM undo_log ORDER BY branch_id")
        .fetch_all(admin).await.unwrap().iter().map(|r|serde_json::json!({"xid":r.get::<String,_>("xid"),"branch_id":r.get::<i64,_>("branch_id"),"context":r.get::<String,_>("context"),"bytes":r.get::<Vec<u8>,_>("rollback_info"),"status":r.get::<i32,_>("log_status"),"created":r.get::<String,_>("created"),"modified":r.get::<String,_>("modified")})).collect()
}
