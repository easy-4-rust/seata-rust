//! 自有 MySQL 的受管理 XA 结构及只读观察，不为生产调用补造登记回执。
#![allow(clippy::unwrap_used, dead_code)]
#[path = "owned_mysql.rs"]
pub mod owned_mysql;
use seata_db_mysql::{MySqlDataSource, MySqlXaResourceManager};
use seata_transport::{
    ConnectionConfig, ConnectionPoolConfig, ReconnectPolicy, RegistrationIdentity,
};
use std::{path::PathBuf, sync::Arc, time::Duration};

/// 无参数；在自有测试库中显式重建 014 表与独立业务表，返回原管理器及原数据源。
pub async fn seed() -> (
    sqlx::MySqlConnection,
    MySqlDataSource,
    Arc<MySqlXaResourceManager>,
) {
    let mut admin = owned_mysql::admin().await;
    sqlx::raw_sql("DROP TABLE IF EXISTS seata_rs_xa_session_guard_v1; DROP TABLE IF EXISTS seata_rs_xa_guard_enrollment_v1; DROP TABLE IF EXISTS seata_rs_xa_prepare_evidence_v1; DROP TABLE IF EXISTS seata_rs_xa_commit_witness_v1; DROP TABLE IF EXISTS seata_rs_xa_branch_v1; DROP TABLE IF EXISTS xa_managed; CREATE TABLE xa_managed(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB").execute(&mut admin).await.unwrap();
    sqlx::raw_sql(include_str!("../../migrations/014_seata_rs_xa_branch.sql"))
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/015_seata_rs_xa_commit_witness.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/016_seata_rs_xa_prepare_evidence.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/017_seata_rs_xa_guard_enrollment.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/018_seata_rs_xa_session_guard.sql"
    ))
    .execute(&mut admin)
    .await
    .unwrap();
    let source = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let adapter = Arc::new(MySqlXaResourceManager::new(source.clone()).unwrap());
    (admin, source, adapter)
}
/// 参数为自有回环端点；返回原 V1 帧预算及有界单槽重连配置。
pub fn pool(endpoint: std::net::SocketAddr) -> ConnectionPoolConfig {
    let connection = ConnectionConfig::new(
        seata_protocol::v1::FrameLimits::new(
            65536,
            32767,
            128,
            seata_protocol::PayloadLimits::new(65536, 512, 1024, 16).unwrap(),
        )
        .unwrap(),
        16,
        Duration::from_secs(3),
        Duration::from_secs(3),
    )
    .unwrap();
    ConnectionPoolConfig::new(
        endpoint,
        connection,
        1,
        16,
        ReconnectPolicy::new(
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        )
        .unwrap(),
    )
    .unwrap()
}
/// 参数为是否连接固定原 Java TC；返回匹配该隔离夹具的原登记身份，不更改生产认证。
pub fn registration(java: bool) -> RegistrationIdentity {
    RegistrationIdentity::new(
        "1.1.0".into(),
        if java { "phase0-auth" } else { "rm-app" }.into(),
        if java { "default_tx_group" } else { "rm-group" }.into(),
        Some(
            if java {
                "phase0-local-only"
            } else {
                "rm-public-fixture"
            }
            .into(),
        ),
    )
    .unwrap()
}
/// 参数为固定证据名；返回仅位于 D/E 的自有输出目录，禁止写开发工作区外其他盘。
pub fn folder(name: &str) -> PathBuf {
    let out = PathBuf::from(std::env::var_os("SEATA_XA_OUT").unwrap());
    assert!(
        out.is_absolute()
            && (out.to_string_lossy().starts_with("D:") || out.to_string_lossy().starts_with("E:"))
    );
    let path = out.join(name);
    std::fs::create_dir_all(&path).unwrap();
    path
}
/// 参数为固定名称与实际观察；记录当前原进程 ID 和结构化现场，不替代独立 SQL。
pub fn record(name: &str, mut value: serde_json::Value) {
    value.as_object_mut().unwrap().insert(
        "rust_process_id".into(),
        serde_json::json!(std::process::id()),
    );
    std::fs::write(
        folder("proof").join(format!("{name}.json")),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}
/// 参数为原独立连接；返回按原主键排序的真实业务值。
pub async fn business(admin: &mut sqlx::MySqlConnection) -> Vec<(i32, i32)> {
    sqlx::query_as("SELECT id,v FROM xa_managed ORDER BY id")
        .fetch_all(admin)
        .await
        .unwrap()
}
