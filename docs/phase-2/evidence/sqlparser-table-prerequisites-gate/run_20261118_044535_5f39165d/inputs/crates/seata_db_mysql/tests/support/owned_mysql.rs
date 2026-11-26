//! 自有容器测试的连接配置；缺少运行器环境必须失败。
use seata_db_mysql::MySqlOptions;
use sqlx::Connection;
pub fn options(port: Option<u16>) -> MySqlOptions {
    MySqlOptions::new(
        "127.0.0.1".into(),
        port.unwrap_or_else(|| std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap()),
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
        std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap()
}
pub async fn admin() -> sqlx::MySqlConnection {
    let config = sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap())
        .username("root")
        .password(&std::env::var("SEATA_MYSQL_PASSWORD").unwrap())
        .database(&std::env::var("SEATA_MYSQL_DATABASE").unwrap())
        .charset("utf8mb4")
        .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled);
    sqlx::MySqlConnection::connect_with(&config).await.unwrap()
}
