//! 自有 MySQL 证书链、主机身份、强制 TLS 与辅助元数据连接的真实握手验证。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_at::SqlRecognizerFactory;
use seata_db_mysql::{MySqlDataSource, MySqlOptions};
use sqlx::{Connection, Row};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires owned MySQL X509 account and client identity files"]
async fn x509_account_requires_client_identity_for_pool_and_auxiliary_sessions() {
    let folder = std::path::PathBuf::from(std::env::var("SEATA_MTLS_CERTS").unwrap());
    let ca = std::fs::read(folder.join("ca.pem")).unwrap();
    let certificate = std::fs::read(folder.join("client.pem")).unwrap();
    let key = std::fs::read(folder.join("client-key.pem")).unwrap();
    let other_certificate = std::fs::read(folder.join("untrusted-client.pem")).unwrap();
    let other_key = std::fs::read(folder.join("untrusted-client-key.pem")).unwrap();
    let mismatch_key = std::fs::read(folder.join("server-key.pem")).unwrap();
    let make_options = |password: &str| {
        MySqlOptions::new(
            "127.0.0.1".into(),
            std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
            "mtls_fixture".into(),
            password.into(),
            std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
        )
        .unwrap()
        .with_tls_ca_pem(ca.clone())
        .unwrap()
        .operation_timeout(Duration::from_secs(5))
        .unwrap()
    };
    let missing = MySqlDataSource::connect(make_options("seata-mysql-fixture-only")).await;
    assert!(
        missing.is_err(),
        "X509 account accepted a connection without a client certificate"
    );
    for (label, cert, private, password) in [
        (
            "untrusted client",
            other_certificate,
            other_key,
            "seata-mysql-fixture-only",
        ),
        (
            "mismatched private key",
            certificate.clone(),
            mismatch_key,
            "seata-mysql-fixture-only",
        ),
        (
            "wrong password",
            certificate.clone(),
            key.clone(),
            "wrong-fixture-password",
        ),
    ] {
        let result = MySqlDataSource::connect(
            make_options(password)
                .with_tls_client_identity_pem(cert, private)
                .unwrap(),
        )
        .await;
        assert!(result.is_err(), "invalid client identity accepted: {label}");
        let error = result.unwrap_err();
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("BEGIN PRIVATE KEY"));
        assert!(!rendered.contains("wrong-fixture-password"));
    }
    let secured = make_options("seata-mysql-fixture-only")
        .with_tls_client_identity_pem(certificate.clone(), key.clone())
        .unwrap();
    assert!(!format!("{secured:?}").contains("BEGIN PRIVATE KEY"));
    let ds = MySqlDataSource::connect(secured)
        .await
        .expect("valid X509 identity must authenticate data source");
    let mut tx = ds.begin().await.unwrap();
    tx.table_meta("x509_work").await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("UPDATE x509_work SET v=11 WHERE id=1").unwrap(),
        &[],
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    let config = sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap())
        .username("mtls_fixture")
        .password("seata-mysql-fixture-only")
        .database(&std::env::var("SEATA_MYSQL_DATABASE").unwrap())
        .ssl_mode(sqlx::mysql::MySqlSslMode::VerifyIdentity)
        .ssl_ca_from_pem(ca)
        .ssl_client_cert_from_pem(certificate)
        .ssl_client_key_from_pem(key);
    let mut observer = sqlx::MySqlConnection::connect_with(&config).await.unwrap();
    let value: i32 = sqlx::query_scalar("SELECT v FROM x509_work WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(value, 10);
    let cipher = sqlx::query("SHOW SESSION STATUS LIKE 'Ssl_cipher'")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert!(!cipher.get::<String, _>("Value").is_empty());
    println!("X509 authenticated pool and auxiliary sessions; independently restored v={value}, cipher={}",cipher.get::<String,_>("Value"));
    ds.close().await.unwrap();
}

fn options(host: &str, ca: Option<Vec<u8>>) -> MySqlOptions {
    let options = MySqlOptions::new(
        host.into(),
        std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
        std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
    )
    .unwrap()
    .operation_timeout(Duration::from_secs(5))
    .unwrap();
    match ca {
        Some(ca) => options.with_tls_ca_pem(ca).unwrap(),
        None => options,
    }
}

#[tokio::test]
#[ignore = "requires owned TLS-only real MySQL fixture and CA files"]
async fn trusted_ca_keeps_hostname_verification_and_all_business_connections_encrypted() {
    let ca = std::fs::read(std::env::var("SEATA_TLS_CA").unwrap()).unwrap();
    let other_ca = std::fs::read(std::env::var("SEATA_TLS_OTHER_CA").unwrap()).unwrap();
    // 服务器证书只有 127.0.0.1 的 IP SAN，localhost 连接仍到达同一回环服务。
    for (host, trust) in [
        ("127.0.0.1", None),
        ("127.0.0.1", Some(other_ca)),
        ("localhost", Some(ca.clone())),
    ] {
        let result = MySqlDataSource::connect(options(host, trust)).await;
        assert!(
            result.is_err(),
            "invalid TLS identity/trust was accepted for {host}"
        );
        let error = result.unwrap_err();
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("seata-mysql-fixture-only"));
        assert!(!rendered.contains("BEGIN CERTIFICATE"));
    }
    let config = sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap())
        .username("root")
        .password(&std::env::var("SEATA_MYSQL_PASSWORD").unwrap())
        .database(&std::env::var("SEATA_MYSQL_DATABASE").unwrap())
        .ssl_mode(sqlx::mysql::MySqlSslMode::VerifyIdentity)
        .ssl_ca_from_pem(ca.clone());
    let mut admin = sqlx::MySqlConnection::connect_with(&config).await.unwrap();
    // 先证明 localhost 能到达同一实例，避免把 DNS/IPv6 连接失败误作主机身份拒绝。
    let mut alias = sqlx::MySqlConnection::connect_with(
        &config
            .clone()
            .host("localhost")
            .ssl_mode(sqlx::mysql::MySqlSslMode::Required),
    )
    .await
    .unwrap();
    let expected_uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    let alias_uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
        .fetch_one(&mut alias)
        .await
        .unwrap();
    assert_eq!(expected_uuid, alias_uuid);
    alias.close().await.unwrap();
    let required: i64 = sqlx::query_scalar("SELECT @@require_secure_transport")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(required, 1);
    let cipher = sqlx::query("SHOW SESSION STATUS LIKE 'Ssl_cipher'")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert!(!cipher.get::<String, _>("Value").is_empty());
    println!(
        "owned MySQL TLS cipher={}, required={}, alias_instance_matches=true",
        cipher.get::<String, _>("Value"),
        required
    );
    sqlx::raw_sql("DROP TABLE IF EXISTS tls_work; CREATE TABLE tls_work(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB; INSERT INTO tls_work VALUES(1,10)")
        .execute(&mut admin).await.unwrap();
    let ds = MySqlDataSource::connect(options("127.0.0.1", Some(ca)))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.table_meta("tls_work").await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("UPDATE tls_work SET v=11 WHERE id=1").unwrap(),
        &[],
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    let original: i32 = sqlx::query_scalar("SELECT v FROM tls_work WHERE id=1")
        .fetch_one(&mut admin)
        .await
        .unwrap();
    assert_eq!(original, 10);
    println!("trusted CA business rollback independently observed v={original}");
    // 同一只允许加密 TCP 的服务器拒绝显式关闭 TLS，证明正向业务和辅助连接未降级。
    assert!(MySqlDataSource::connect(
        options("127.0.0.1", None)
            .with_insecure_loopback_tls()
            .unwrap()
    )
    .await
    .is_err());
    ds.close().await.unwrap();
}
