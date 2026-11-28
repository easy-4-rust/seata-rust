//! 原 MySQL 已确认 COMMIT 而响应被自有代理截留的无变化一阶段故障测试。
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/callback_peer.rs"]
#[allow(dead_code)]
mod callback_peer;
#[path = "support/first_phase_fixture.rs"]
#[allow(dead_code)]
mod first_fixture;
#[path = "support/undo_store_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;
#[path = "support/rm_recovery_fixture.rs"]
#[allow(dead_code)]
mod recovery;
#[path = "../../seata_rm/tests/support/rm_peer.rs"]
mod rm_fixture;

use first_fixture::Harness;
use seata_at::{MySqlJdbcProfile, SqlRecognizerFactory};
use seata_db_mysql::{DbError, MySqlDataSource, MySqlDataSourceManager, MySqlNoChangeCommitError};
use std::{path::PathBuf, time::Duration};

#[tokio::test]
#[ignore = "requires owned real MySQL and armed response-loss proxy"]
async fn server_ok_withheld_from_original_commit_is_bounded_unknown() {
    let mut h = Harness::start(Duration::from_secs(5), Duration::from_secs(2)).await;
    let proxy_port: u16 = std::env::var("SEATA_MYSQL_COMMIT_PROXY_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let arm_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_ARM").unwrap());
    let ok_file = PathBuf::from(std::env::var("SEATA_MYSQL_COMMIT_PROXY_OK").unwrap());
    assert!(!arm_file.exists() && !ok_file.exists());
    let mut socket = h.accept().await;
    let source = MySqlDataSource::connect(
        owned_mysql::options(Some(proxy_port))
            .operation_timeout(Duration::from_millis(350))
            .unwrap(),
    )
    .await
    .unwrap();
    let adapter =
        MySqlDataSourceManager::new(source.clone(), MySqlJdbcProfile::Connector8Default).unwrap();
    let mut transaction = source.begin().await.unwrap();
    assert_eq!(
        transaction
            .execute(
                &SqlRecognizerFactory::recognize("UPDATE undo_work SET v='absent' WHERE id=99")
                    .unwrap(),
                &[]
            )
            .await
            .unwrap(),
        0
    );
    let checked = adapter
        .prepare_transaction(
            transaction,
            &h.rm.client(),
            "commit-no-change:server-ok-lost",
            None,
        )
        .await
        .unwrap()
        .lock_commit_preconditions()
        .await
        .unwrap();
    assert!(checked.registered_branch().is_none());
    sqlx::query("SET GLOBAL log_output='TABLE'")
        .execute(&mut h.admin)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL general_log=ON")
        .execute(&mut h.admin)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    std::fs::write(&arm_file, b"armed\n").unwrap();
    let outcome = tokio::time::timeout(Duration::from_millis(1500), checked.commit_no_change())
        .await
        .expect("inner commit deadline must finish before outer watchdog");
    assert!(matches!(
        outcome,
        Err(MySqlNoChangeCommitError::Unknown {
            source: DbError::Deadline {
                operation: "no-change COMMIT"
            }
        })
    ));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !ok_file.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "original MySQL OK missing"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        std::fs::read_to_string(&ok_file).unwrap().trim_end(),
        "original MySQL COMMIT returned OK; reply held"
    );
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mysql.general_log WHERE command_type='Query' AND argument='COMMIT'",
    )
    .fetch_one(&mut h.admin)
    .await
    .unwrap();
    sqlx::query("SET GLOBAL general_log=OFF")
        .execute(&mut h.admin)
        .await
        .unwrap();
    assert_eq!(after, before + 1);
    println!("server COMMIT OK withheld; general_log delta=1; local result=bounded unknown");
    source.close().await.unwrap();
    assert_eq!(h.closed_counts(&mut socket).await, (0, 0, (0, 0, 0)));
}
