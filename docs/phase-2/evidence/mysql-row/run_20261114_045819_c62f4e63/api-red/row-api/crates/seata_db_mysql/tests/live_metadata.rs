//! 仅由拥有自有MySQL容器的测试驱动器显式执行，缺环境必须失败而不能跳过。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, ColumnType, DecimalValue, Field, FieldValue,
};
use seata_db_mysql::{DbError, MySqlDataSource, MySqlOptions};
use sqlx::{Connection, Row};
use std::time::Duration;
#[path = "support/stall_proxy.rs"]
mod stall_proxy;

fn options(user: &str) -> MySqlOptions {
    MySqlOptions::new(
        "127.0.0.1".into(),
        std::env::var("SEATA_MYSQL_PORT")
            .expect("owned port")
            .parse()
            .unwrap(),
        user.into(),
        std::env::var("SEATA_MYSQL_PASSWORD").expect("owned password"),
        std::env::var("SEATA_MYSQL_DATABASE").expect("owned database"),
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap()
}
async fn admin() -> sqlx::MySqlConnection {
    let port: u16 = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let cfg = sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username("root")
        .password(&std::env::var("SEATA_MYSQL_PASSWORD").unwrap())
        .database(&std::env::var("SEATA_MYSQL_DATABASE").unwrap())
        .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled);
    sqlx::MySqlConnection::connect_with(&cfg).await.unwrap()
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn actual_metadata_preserves_physical_integer_text_binary_and_unsigned_decimal_domains() {
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let table = tx.table_meta("physical_types").await.unwrap();
    for (name, bits, unsigned) in [
        ("s8", 8, false),
        ("s16", 16, false),
        ("s24", 24, false),
        ("s32", 32, false),
        ("u8", 8, true),
        ("u16", 16, true),
        ("u24", 24, true),
        ("u32", 32, true),
        ("id", 64, false),
    ] {
        let c = table.column(name).unwrap();
        assert_eq!(c.domain(), Some(&ColumnDomain::Integer { bits, unsigned }));
    }
    assert_eq!(
        table.column("txt").unwrap().domain(),
        Some(&ColumnDomain::Text {
            charset: CharacterSet::Utf8Mb4,
            character_limit: Some(1),
            fixed_width: false
        })
    );
    assert_eq!(
        table.column("fixed_text").unwrap().domain(),
        Some(&ColumnDomain::Text {
            charset: CharacterSet::Utf8Mb4,
            character_limit: Some(3),
            fixed_width: true
        })
    );
    assert_eq!(
        table.column("text_lob").unwrap().domain(),
        Some(&ColumnDomain::Text {
            charset: CharacterSet::Utf8Mb4,
            character_limit: None,
            fixed_width: false
        })
    );
    assert_eq!(
        table.column("fixed_binary").unwrap().domain(),
        Some(&ColumnDomain::Binary { fixed_width: true })
    );
    assert_eq!(
        table.column("unsigned_decimal").unwrap().domain(),
        Some(&ColumnDomain::Decimal { unsigned: true })
    );
    for (name, value) in [
        ("s8", FieldValue::Signed(128)),
        ("u8", FieldValue::Unsigned(256)),
        ("txt", FieldValue::Text("ab".into())),
        ("txt3", FieldValue::Text("🦀".into())),
        ("ascii_col", FieldValue::Text("é".into())),
        ("fixed_binary", FieldValue::Binary(vec![255])),
        (
            "unsigned_decimal",
            FieldValue::Decimal(DecimalValue::new("-0.01").unwrap()),
        ),
    ] {
        assert!(
            Field::new(table.column(name).unwrap(), value).is_err(),
            "physical domain accepted {name}"
        );
    }
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn exact_metadata_has_physical_order_composite_primary_order_and_unsigned_precision() {
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    assert!(ds.server_version().starts_with("8.4."));
    let mut tx = ds.begin().await.unwrap();
    let table = tx.table_meta("account").await.unwrap();
    assert_eq!(
        table
            .columns()
            .iter()
            .map(ColumnMeta::name)
            .collect::<Vec<_>>(),
        ["b", "a", "amount", "note", "payload", "unsigned_id"]
    );
    assert_eq!(
        table
            .primary_key_columns()
            .map(ColumnMeta::name)
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(
        table.column("UNSIGNED_ID").unwrap().kind(),
        ColumnType::UnsignedInteger
    );
    assert_eq!(
        table.column("amount").unwrap().numeric_precision(),
        Some(65)
    );
    assert_eq!(table.column("amount").unwrap().numeric_scale(), Some(30));
    assert_eq!(table.column("payload").unwrap().kind(), ColumnType::Binary);
    assert!(table.column("note").unwrap().nullable());
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn unsafe_tables_and_unknown_types_fail_closed_and_poison_the_local_scope() {
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    for name in [
        "no_pk",
        "non_transactional",
        "generated_col",
        "auto_id",
        "with_trigger",
        "fk_parent",
        "fk_child",
        "float_col",
        "invisible_col",
        "account_view",
        "undo_log",
    ] {
        let mut tx = ds.begin().await.unwrap();
        assert!(tx.table_meta(name).await.is_err(), "accepted {name}");
        assert!(matches!(
            tx.table_meta("account").await,
            Err(DbError::Poisoned)
        ));
        tx.rollback().await.unwrap();
    }
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn metadata_lock_blocks_actual_ddl_until_the_same_transaction_rolls_back() {
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.table_meta("mdl_guard").await.unwrap();
    let mut other = admin().await;
    sqlx::query("SET SESSION lock_wait_timeout=1")
        .execute(&mut other)
        .await
        .unwrap();
    let error = sqlx::query("ALTER TABLE mdl_guard ADD COLUMN extra INT")
        .execute(&mut other)
        .await
        .unwrap_err();
    assert_eq!(
        error
            .as_database_error()
            .unwrap()
            .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
            .unwrap()
            .number(),
        1205
    );
    tx.rollback().await.unwrap();
    sqlx::query("ALTER TABLE mdl_guard ADD COLUMN extra INT")
        .execute(&mut other)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE mdl_guard DROP COLUMN extra")
        .execute(&mut other)
        .await
        .unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn drop_releases_real_metadata_lock_without_a_business_commit() {
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.table_meta("drop_guard").await.unwrap();
    drop(tx);
    let mut other = admin().await;
    sqlx::query("SET SESSION lock_wait_timeout=2")
        .execute(&mut other)
        .await
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(3),
        sqlx::query("ALTER TABLE drop_guard ADD COLUMN extra INT").execute(&mut other),
    )
    .await
    .unwrap()
    .unwrap();
    sqlx::query("ALTER TABLE drop_guard DROP COLUMN extra")
        .execute(&mut other)
        .await
        .unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn metadata_visibility_is_required_before_interpreting_missing_triggers_as_absence() {
    let ds = MySqlDataSource::connect(options("metadata_low"))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    assert!(matches!(
        tx.table_meta("with_trigger").await,
        Err(DbError::Unsupported {
            feature: "metadata trigger visibility"
        })
    ));
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    let mut other = admin().await;
    let count=sqlx::query("SELECT COUNT(*) AS n FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA=DATABASE() AND EVENT_OBJECT_TABLE='with_trigger'")
        .fetch_one(&mut other).await.unwrap().try_get::<i64,_>("n").unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn schema_partial_revocation_cannot_be_overridden_by_a_global_trigger_grant() {
    let ds = MySqlDataSource::connect(options("metadata_revoked"))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let result = tx.table_meta("with_trigger").await;
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    assert!(
        matches!(
            result,
            Err(DbError::Unsupported {
                feature: "partial metadata privilege revocation"
            })
        ),
        "partial revoke was not rejected: {result:?}"
    );
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn scoped_table_grants_do_not_prove_global_inbound_foreign_key_visibility() {
    let ds = MySqlDataSource::connect(options("metadata_scoped"))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let result = tx.table_meta("account").await;
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
    assert!(matches!(
        result,
        Err(DbError::Unsupported {
            feature: "metadata foreign-key visibility"
        })
    ));
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn withheld_database_response_obeys_the_client_deadline_and_discards_the_transaction() {
    let upstream = std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap();
    let proxy = stall_proxy::StallProxy::start(upstream).await;
    let cfg = MySqlOptions::new(
        "127.0.0.1".into(),
        proxy.port,
        "root".into(),
        std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
        std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
    )
    .unwrap()
    .with_insecure_loopback_tls()
    .unwrap();
    let ds = MySqlDataSource::connect(cfg).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    proxy.stall();
    let result =
        tokio::time::timeout(Duration::from_secs(4), tx.table_meta("deadline_guard")).await;
    let held = proxy.held_bytes();
    drop(tx);
    proxy.shutdown().await;
    let cleanup = ds.close().await;
    println!(
        "MYSQL_PROXY_HELD bytes={held} cleanup_ok={}",
        cleanup.is_ok()
    );
    assert!(
        held > 0,
        "fixture must actually withhold original server response bytes"
    );
    assert!(
        matches!(
            result,
            Ok(Err(DbError::Deadline {
                operation: "table metadata"
            }))
        ),
        "client deadline missing: {result:?}"
    );
    assert!(cleanup.is_ok());
}
