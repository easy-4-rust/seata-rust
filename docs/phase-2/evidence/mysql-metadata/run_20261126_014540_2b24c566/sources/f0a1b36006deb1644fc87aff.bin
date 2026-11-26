//! 自有 MySQL 的证书有效期、在线叶证书及 CA 轮换，不修改生产证书或系统时钟。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::{DbError, MySqlDataSource, MySqlOptions};
use sqlx::{Connection, Row};
use std::{path::PathBuf, time::Duration};

// 同一夹具的 TLS 上下文与活动证书文件是共享资源，测试入口自身也必须串行。
static TLS_CONTEXT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn folder() -> PathBuf {
    let path = PathBuf::from(std::env::var("SEATA_TLS_LIFECYCLE_CERTS").unwrap());
    assert!(path.is_absolute() && path.starts_with("D:/codex-builds"));
    path
}
fn read(name: &str) -> Vec<u8> {
    std::fs::read(folder().join(name)).unwrap()
}
fn options(ca: Vec<u8>, user: &str) -> MySqlOptions {
    options_at(
        ca,
        user,
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
    )
}
fn options_at(ca: Vec<u8>, user: &str, port: u16) -> MySqlOptions {
    MySqlOptions::new(
        "127.0.0.1".into(),
        port,
        user.into(),
        "seata-mysql-fixture-only".into(),
        std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
    )
    .unwrap()
    .with_tls_ca_pem(ca)
    .unwrap()
    .operation_timeout(Duration::from_secs(5))
    .unwrap()
}
async fn admin() -> sqlx::MySqlConnection {
    let config = sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap())
        .username("root")
        .password("seata-mysql-fixture-only")
        .database(&std::env::var("SEATA_MYSQL_DATABASE").unwrap())
        .ssl_mode(sqlx::mysql::MySqlSslMode::VerifyIdentity)
        .ssl_ca_from_pem(read("ca.pem"));
    sqlx::MySqlConnection::connect_with(&config).await.unwrap()
}
async fn reload(admin: &mut sqlx::MySqlConnection, certificate: &str, key: &str, ca: &str) {
    let path = folder();
    std::fs::copy(path.join(certificate), path.join("active-server.pem")).unwrap();
    std::fs::copy(path.join(key), path.join("active-server-key.pem")).unwrap();
    std::fs::copy(path.join(ca), path.join("active-ca.pem")).unwrap();
    sqlx::raw_sql("ALTER INSTANCE RELOAD TLS")
        .execute(&mut *admin)
        .await
        .unwrap();
    let row = sqlx::query("SHOW GLOBAL STATUS LIKE 'Ssl_server_not_after'")
        .fetch_one(admin)
        .await
        .unwrap();
    println!(
        "TLS reload certificate={certificate}, server_not_after={}",
        row.get::<String, _>("Value")
    );
}
async fn assert_original(admin: &mut sqlx::MySqlConnection) {
    let value: i32 = sqlx::query_scalar("SELECT v FROM lifecycle_work WHERE id=1")
        .fetch_one(admin)
        .await
        .unwrap();
    assert_eq!(value, 10);
    println!("independent lifecycle business value={value}");
}
async fn assert_new_metadata_session(source: &MySqlDataSource) {
    let mut tx = source.begin().await.unwrap();
    tx.table_meta("lifecycle_work").await.unwrap();
    tx.rollback().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned reloadable TLS MySQL fixture and lifecycle certificates"]
async fn expired_and_future_server_certificates_reject_new_and_auxiliary_sessions() {
    let _lease = TLS_CONTEXT.lock().await;
    let mut observer = admin().await;
    let uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    let port = std::env::var("SEATA_TLS_LIFECYCLE_BRIDGE_PORT")
        .unwrap()
        .parse()
        .unwrap();
    for variant in ["server-expired.pem", "server-future.pem"] {
        std::fs::copy(
            folder().join("server.pem"),
            folder().join("bridge-active.pem"),
        )
        .unwrap();
        let ds = MySqlDataSource::connect(options_at(read("ca.pem"), "root", port))
            .await
            .unwrap();
        let mut held = ds.begin().await.unwrap();
        held.execute(
            &SqlRecognizerFactory::recognize("UPDATE lifecycle_work SET v=11 WHERE id=1").unwrap(),
            &[],
        )
        .await
        .unwrap();
        std::fs::copy(folder().join(variant), folder().join("bridge-active.pem")).unwrap();
        let still_uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
            .fetch_one(&mut observer)
            .await
            .unwrap();
        assert_eq!(uuid, still_uuid);
        assert!(
            MySqlDataSource::connect(options_at(read("ca.pem"), "root", port))
                .await
                .is_err(),
            "invalid server certificate lifetime was accepted: {variant}"
        );
        // 既有业务连接继续存在，但本次新辅助握手必须拒绝并污染整个业务作用域。
        assert!(
            held.execute(
                &SqlRecognizerFactory::recognize("UPDATE lifecycle_work SET v=12 WHERE id=1")
                    .unwrap(),
                &[]
            )
            .await
            .is_err(),
            "auxiliary session accepted invalid certificate lifetime: {variant}"
        );
        assert!(matches!(
            held.table_meta("lifecycle_work").await,
            Err(DbError::Poisoned)
        ));
        held.rollback().await.unwrap();
        assert_original(&mut observer).await;
        println!("server lifetime rejected new and auxiliary sessions: {variant}; old observer stayed connected");
        std::fs::copy(
            folder().join("server.pem"),
            folder().join("bridge-active.pem"),
        )
        .unwrap();
        assert_new_metadata_session(&ds).await;
        ds.close().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "requires owned reloadable TLS MySQL fixture and lifecycle certificates"]
async fn mysql_refuses_invalid_lifetime_reload_and_keeps_previous_tls_context() {
    let _lease = TLS_CONTEXT.lock().await;
    let mut observer = admin().await;
    for variant in ["server-expired.pem", "server-future.pem"] {
        std::fs::copy(folder().join(variant), folder().join("active-server.pem")).unwrap();
        let error = sqlx::raw_sql("ALTER INSTANCE RELOAD TLS")
            .execute(&mut observer)
            .await
            .unwrap_err();
        assert_eq!(
            error
                .as_database_error()
                .unwrap()
                .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
                .unwrap()
                .number(),
            3888
        );
        let ds = MySqlDataSource::connect(options(read("ca.pem"), "root"))
            .await
            .unwrap();
        assert_new_metadata_session(&ds).await;
        assert_original(&mut observer).await;
        println!("MySQL rejected invalid reload with 3888 and kept previous context: {variant}");
        reload(&mut observer, "server.pem", "server-key.pem", "ca.pem").await;
        ds.close().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "requires owned reloadable TLS MySQL fixture and lifecycle certificates"]
async fn leaf_and_ca_rotation_keep_new_connection_trust_and_original_transaction_atomicity() {
    let _lease = TLS_CONTEXT.lock().await;
    let mut observer = admin().await;
    let ds = MySqlDataSource::connect(options(read("ca.pem"), "root"))
        .await
        .unwrap();
    let mut held = ds.begin().await.unwrap();
    held.execute(
        &SqlRecognizerFactory::recognize("UPDATE lifecycle_work SET v=11 WHERE id=1").unwrap(),
        &[],
    )
    .await
    .unwrap();
    reload(&mut observer, "renewed.pem", "renewed-key.pem", "ca.pem").await;
    assert_new_metadata_session(&ds).await;
    held.execute(
        &SqlRecognizerFactory::recognize("UPDATE lifecycle_work SET v=12 WHERE id=1").unwrap(),
        &[],
    )
    .await
    .unwrap();
    println!("same CA renewed leaf accepted by new pool and auxiliary connections");
    reload(
        &mut observer,
        "rotated.pem",
        "rotated-key.pem",
        "rotated-ca.pem",
    )
    .await;
    assert!(
        ds.begin().await.is_err(),
        "old CA pool accepted new CA server"
    );
    assert!(
        held.execute(
            &SqlRecognizerFactory::recognize("UPDATE lifecycle_work SET v=13 WHERE id=1").unwrap(),
            &[]
        )
        .await
        .is_err(),
        "old CA auxiliary session accepted new CA server"
    );
    assert!(matches!(
        held.table_meta("lifecycle_work").await,
        Err(DbError::Poisoned)
    ));
    held.rollback().await.unwrap();
    assert_original(&mut observer).await;
    let mut combined = read("ca.pem");
    combined.extend_from_slice(&read("rotated-ca.pem"));
    let overlap = MySqlDataSource::connect(options(combined, "root"))
        .await
        .unwrap();
    assert_new_metadata_session(&overlap).await;
    let replacement = MySqlDataSource::connect(options(read("rotated-ca.pem"), "root"))
        .await
        .unwrap();
    assert_new_metadata_session(&replacement).await;
    println!("CA rotation rejected old trust, accepted overlap and replacement trust; old observer stayed connected");
    reload(&mut observer, "server.pem", "server-key.pem", "ca.pem").await;
    assert_new_metadata_session(&ds).await;
    assert_new_metadata_session(&overlap).await;
    println!("same overlap trust bundle accepted fresh sessions under both CAs");
    overlap.close().await.unwrap();
    replacement.close().await.unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned X509 MySQL account and lifecycle client certificates"]
async fn expired_and_future_client_certificates_are_rejected_by_x509_account() {
    let _lease = TLS_CONTEXT.lock().await;
    let mut observer = admin().await;
    for variant in ["client-expired.pem", "client-future.pem"] {
        let candidate = options(read("ca.pem"), "mtls_fixture")
            .with_tls_client_identity_pem(read(variant), read("client-key.pem"))
            .unwrap();
        assert!(
            MySqlDataSource::connect(candidate).await.is_err(),
            "invalid client certificate lifetime was accepted: {variant}"
        );
        println!("X509 rejected client lifetime: {variant}");
        assert_original(&mut observer).await;
    }
    let candidate = options(read("ca.pem"), "mtls_fixture")
        .with_tls_client_identity_pem(read("client.pem"), read("client-key.pem"))
        .unwrap();
    let ds = MySqlDataSource::connect(candidate).await.unwrap();
    assert_new_metadata_session(&ds).await;
    println!("valid client identity accepted after lifetime rejection controls");
    ds.close().await.unwrap();
}
