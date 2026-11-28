//! 固定原 Java TC 联合夹具中 MySQL 服务端 TLS 身份的准入检查。

use crate::{fixture, java_pool_probe};
use seata_db_mysql::{DbError, MySqlDataSource, MySqlOptions};
use std::path::{Path, PathBuf};

/// 用无关 CA 尝试连接，等待独立数据库核验，再返回同一地址和数据库的正确信任配置。
pub(super) async fn admit(observations: &Path, xid: &str) -> MySqlOptions {
    let material = PathBuf::from(std::env::var_os("SEATA_JAVA_MYSQL_TLS").unwrap());
    let options = MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
        std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
    )
    .unwrap();
    let wrong = options
        .clone()
        .with_tls_ca_pem(std::fs::read(material.join("wrong-ca.pem")).unwrap())
        .unwrap();
    let result = MySqlDataSource::connect(wrong).await;
    assert!(
        matches!(
            result,
            Err(DbError::Driver {
                operation: "connect",
                sql_state: None,
                code: None,
                ..
            })
        ),
        "wrong CA must reject MySQL during the driver connect call"
    );
    fixture::save(
        "java-mysql-tls-rejected.json",
        &serde_json::json!({"xid":xid, "rust_connect_error":"Driver/connect"}),
    );
    java_pool_probe::wait_for_snapshot(observations, "java-mysql-tls-permitted").await;
    options
        .with_tls_ca_pem(std::fs::read(material.join("ca.pem")).unwrap())
        .unwrap()
}
