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
        "FK_PARENT",
        "fk$dollar_parent",
        "fk$dollar_child",
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
async fn definer_channel_checks_hidden_triggers_and_cross_schema_foreign_keys() {
    let ds = MySqlDataSource::connect(options("metadata_executor"))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    assert_eq!(tx.table_meta("account").await.unwrap().name(), "account");
    tx.rollback().await.unwrap();

    for (table, reason) in [
        ("with_trigger", "table trigger"),
        ("definer_fk_parent", "inbound/outbound foreign key"),
    ] {
        let mut tx = ds.begin().await.unwrap();
        let result = tx.table_meta(table).await;
        assert!(
            matches!(result, Err(DbError::Unsupported { feature }) if feature == reason),
            "{result:?}"
        );
        tx.rollback().await.unwrap();
    }
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn definer_channel_rejects_partial_revokes_and_missing_scope() {
    let mut root = admin().await;
    let database = std::env::var("SEATA_MYSQL_DATABASE").unwrap();
    let external =
        seata_datasource::TableMeta::quote_identifier(&format!("{database}_external")).unwrap();
    let own = seata_datasource::TableMeta::quote_identifier(&database).unwrap();
    let definer = "'metadata_definer'@'%'";
    let ds = MySqlDataSource::connect(options("metadata_executor"))
        .await
        .unwrap();
    for (revoke, restore, reason) in [
        (
            format!("REVOKE REFERENCES ON {external}.* FROM {definer}"),
            format!("GRANT REFERENCES ON {external}.* TO {definer}"),
            "partial metadata definer privilege revocation",
        ),
        (
            format!("REVOKE TRIGGER ON {own}.* FROM {definer}"),
            format!("GRANT TRIGGER ON {own}.* TO {definer}"),
            "metadata constraint definer privileges",
        ),
    ] {
        sqlx::query(&revoke).execute(&mut root).await.unwrap();
        let mut tx = ds.begin().await.unwrap();
        let result = tx.table_meta("account").await;
        tx.rollback().await.unwrap();
        sqlx::query(&restore).execute(&mut root).await.unwrap();
        assert!(
            matches!(result, Err(DbError::Unsupported { feature }) if feature == reason),
            "{result:?}"
        );
    }
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn table_trigger_grants_only_cover_their_exact_table() {
    let ds = MySqlDataSource::connect(options("metadata_table_scope"))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    assert_eq!(tx.table_meta("account").await.unwrap().name(), "account");
    tx.rollback().await.unwrap();

    let mut tx = ds.begin().await.unwrap();
    assert!(matches!(
        tx.table_meta("with_trigger").await,
        Err(DbError::Unsupported {
            feature: "table trigger"
        })
    ));
    tx.rollback().await.unwrap();

    let mut tx = ds.begin().await.unwrap();
    assert!(matches!(
        tx.table_meta("deadline_guard").await,
        Err(DbError::Unsupported {
            feature: "metadata trigger visibility"
        })
    ));
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
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

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn later_table_ddl_uses_current_primary_key_and_column_types_in_an_existing_transaction() {
    use seata_at::SqlRecognizerFactory;

    let mut observer = admin().await;
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS ddl_first,ddl_later;
         CREATE TABLE ddl_first(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
         CREATE TABLE ddl_later(id INT NOT NULL,alternate INT NOT NULL,v INT NOT NULL,PRIMARY KEY(id)) ENGINE=InnoDB;
         INSERT INTO ddl_first VALUES(1,10);
         INSERT INTO ddl_later VALUES(1,7,10);",
    ).execute(&mut observer).await.unwrap();
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    assert_eq!(
        tx.execute(
            &SqlRecognizerFactory::recognize("UPDATE ddl_first SET v=11 WHERE id=1").unwrap(),
            &[]
        )
        .await
        .unwrap(),
        1
    );
    // 原事务已读过表 A 的字典和镜像。表 B 此前未被它打开，管理员可独立修改定义。
    sqlx::query(
        "ALTER TABLE ddl_later DROP PRIMARY KEY, ADD PRIMARY KEY(alternate), MODIFY v BIGINT UNSIGNED NOT NULL",
    ).execute(&mut observer).await.unwrap();
    let observed = tx.table_meta("ddl_later").await;
    assert!(matches!(
        observed,
        Err(DbError::Unsupported {
            feature: "table definition changed after transaction metadata snapshot",
        })
    ));
    assert!(matches!(
        tx.table_meta("ddl_first").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let meta = tx.table_meta("ddl_later").await.unwrap();
    assert_eq!(
        meta.primary_key_columns()
            .map(ColumnMeta::name)
            .collect::<Vec<_>>(),
        ["alternate"]
    );
    assert_eq!(
        meta.column("v").unwrap().kind(),
        ColumnType::UnsignedInteger
    );
    assert_eq!(
        tx.execute(
            &SqlRecognizerFactory::recognize("UPDATE ddl_later SET v=? WHERE alternate=7").unwrap(),
            &[FieldValue::Unsigned(u64::MAX)],
        )
        .await
        .unwrap(),
        1
    );
    let undo = &tx.undo_logs()[0];
    assert_eq!(
        undo.after_image().rows()[0].field("v").unwrap().value(),
        &FieldValue::Unsigned(u64::MAX)
    );
    assert_eq!(
        undo.after_image().rows()[0]
            .primary_keys()
            .next()
            .unwrap()
            .column()
            .name(),
        "alternate"
    );
    tx.rollback().await.unwrap();
    let originals: (i32, u64) = sqlx::query_as(
        "SELECT (SELECT v FROM ddl_first WHERE id=1),v FROM ddl_later WHERE alternate=7",
    )
    .fetch_one(&mut observer)
    .await
    .unwrap();
    assert_eq!(originals, (10, 10));
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn type_only_ddl_is_rejected_after_another_table_establishes_the_dictionary_snapshot() {
    use seata_at::SqlRecognizerFactory;

    let mut observer = admin().await;
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS ddl_first,ddl_later;
         CREATE TABLE ddl_first(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
         CREATE TABLE ddl_later(id INT NOT NULL,alternate INT NOT NULL,v INT NOT NULL,PRIMARY KEY(id)) ENGINE=InnoDB;
         INSERT INTO ddl_first VALUES(1,10);
         INSERT INTO ddl_later VALUES(1,7,10);",
    ).execute(&mut observer).await.unwrap();
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    assert_eq!(
        tx.execute(
            &SqlRecognizerFactory::recognize("UPDATE ddl_first SET v=11 WHERE id=1").unwrap(),
            &[]
        )
        .await
        .unwrap(),
        1
    );
    // 原事务已读过表 A 的字典和镜像。表 B 此前未被它打开，管理员可独立修改定义。
    sqlx::query("ALTER TABLE ddl_later MODIFY v BIGINT UNSIGNED NOT NULL")
        .execute(&mut observer)
        .await
        .unwrap();
    let observed = tx.table_meta("ddl_later").await;
    assert!(matches!(
        observed,
        Err(DbError::Unsupported {
            feature: "table definition changed after transaction metadata snapshot",
        })
    ));
    assert!(matches!(
        tx.table_meta("ddl_first").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    let meta = tx.table_meta("ddl_later").await.unwrap();
    assert_eq!(
        meta.primary_key_columns()
            .map(ColumnMeta::name)
            .collect::<Vec<_>>(),
        ["id"]
    );
    assert_eq!(
        meta.column("v").unwrap().kind(),
        ColumnType::UnsignedInteger
    );
    assert_eq!(
        tx.execute(
            &SqlRecognizerFactory::recognize("UPDATE ddl_later SET v=? WHERE id=1").unwrap(),
            &[FieldValue::Unsigned(u64::MAX)],
        )
        .await
        .unwrap(),
        1
    );
    let undo = &tx.undo_logs()[0];
    assert_eq!(
        undo.after_image().rows()[0].field("v").unwrap().value(),
        &FieldValue::Unsigned(u64::MAX)
    );
    assert_eq!(
        undo.after_image().rows()[0]
            .primary_keys()
            .next()
            .unwrap()
            .column()
            .name(),
        "id"
    );
    tx.rollback().await.unwrap();
    let originals: (i32, u64) =
        sqlx::query_as("SELECT (SELECT v FROM ddl_first WHERE id=1),v FROM ddl_later WHERE id=1")
            .fetch_one(&mut observer)
            .await
            .unwrap();
    assert_eq!(originals, (10, 10));
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn engine_only_ddl_is_rejected_before_nontransactional_business_execution() {
    use seata_at::SqlRecognizerFactory;

    let mut observer = admin().await;
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS engine_first,engine_later;
         CREATE TABLE engine_first(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
         CREATE TABLE engine_later(id INT PRIMARY KEY,v INT NOT NULL COMMENT 'line one
) ENGINE=InnoDB fake
line three') ENGINE=InnoDB;
         INSERT INTO engine_first VALUES(1,10);
         INSERT INTO engine_later VALUES(1,10);",
    )
    .execute(&mut observer)
    .await
    .unwrap();
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    assert_eq!(
        tx.execute(
            &SqlRecognizerFactory::recognize("UPDATE engine_first SET v=11 WHERE id=1").unwrap(),
            &[]
        )
        .await
        .unwrap(),
        1
    );
    sqlx::query("ALTER TABLE engine_later ENGINE=MyISAM")
        .execute(&mut observer)
        .await
        .unwrap();
    assert!(matches!(
        tx.table_meta("engine_later").await,
        Err(DbError::Unsupported {
            feature: "current physical engine must be InnoDB",
        })
    ));
    assert!(matches!(
        tx.execute(
            &SqlRecognizerFactory::recognize("UPDATE engine_later SET v=99 WHERE id=1").unwrap(),
            &[]
        )
        .await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let counts: (i32, i32) = sqlx::query_as(
        "SELECT (SELECT v FROM engine_first WHERE id=1),(SELECT v FROM engine_later WHERE id=1)",
    )
    .fetch_one(&mut observer)
    .await
    .unwrap();
    assert_eq!(counts, (10, 10));
    sqlx::query("ALTER TABLE engine_later ENGINE=InnoDB")
        .execute(&mut observer)
        .await
        .unwrap();
    let mut restored = ds.begin().await.unwrap();
    assert_eq!(
        restored
            .execute(
                &SqlRecognizerFactory::recognize("UPDATE engine_later SET v=12 WHERE id=1")
                    .unwrap(),
                &[]
            )
            .await
            .unwrap(),
        1
    );
    restored.rollback().await.unwrap();
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn charset_only_ddl_rejects_stale_text_domain_in_an_existing_transaction() {
    use seata_at::SqlRecognizerFactory;

    let mut observer = admin().await;
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS charset_first,charset_later;
         CREATE TABLE charset_first(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
         CREATE TABLE charset_later(id INT PRIMARY KEY,v VARCHAR(8) CHARACTER SET utf8mb4 NOT NULL) ENGINE=InnoDB;
         INSERT INTO charset_first VALUES(1,10);
         INSERT INTO charset_later VALUES(1,'original');",
    ).execute(&mut observer).await.unwrap();
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("UPDATE charset_first SET v=11 WHERE id=1").unwrap(),
        &[],
    )
    .await
    .unwrap();
    sqlx::query("ALTER TABLE charset_later MODIFY v VARCHAR(8) CHARACTER SET ascii NOT NULL")
        .execute(&mut observer)
        .await
        .unwrap();
    assert!(matches!(
        tx.table_meta("charset_later").await,
        Err(DbError::Unsupported {
            feature: "table definition changed after transaction metadata snapshot",
        })
    ));
    assert!(matches!(
        tx.table_meta("charset_first").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let mut fresh = ds.begin().await.unwrap();
    let meta = fresh.table_meta("charset_later").await.unwrap();
    assert!(matches!(
        meta.column("v").unwrap().domain(),
        Some(ColumnDomain::Text {
            charset: CharacterSet::Ascii,
            ..
        })
    ));
    fresh
        .execute(
            &SqlRecognizerFactory::recognize("UPDATE charset_later SET v='changed' WHERE id=1")
                .unwrap(),
            &[],
        )
        .await
        .unwrap();
    fresh.rollback().await.unwrap();
    let originals: (i32, String) = sqlx::query_as(
        "SELECT (SELECT v FROM charset_first WHERE id=1),v FROM charset_later WHERE id=1",
    )
    .fetch_one(&mut observer)
    .await
    .unwrap();
    assert_eq!(originals, (10, "original".into()));
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn independent_column_shape_ddl_rejects_stale_count_order_and_nullability() {
    use seata_at::SqlRecognizerFactory;

    let mut observer = admin().await;
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    for change in [
        "ADD COLUMN extra INT NULL",
        "MODIFY v INT NOT NULL FIRST",
        "MODIFY v INT NULL",
    ] {
        sqlx::raw_sql(
            "DROP TABLE IF EXISTS shape_first,shape_later;
             CREATE TABLE shape_first(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             CREATE TABLE shape_later(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             INSERT INTO shape_first VALUES(1,10);
             INSERT INTO shape_later VALUES(1,10);",
        )
        .execute(&mut observer)
        .await
        .unwrap();
        let mut tx = ds.begin().await.unwrap();
        tx.execute(
            &SqlRecognizerFactory::recognize("UPDATE shape_first SET v=11 WHERE id=1").unwrap(),
            &[],
        )
        .await
        .unwrap();
        sqlx::query(&format!("ALTER TABLE shape_later {change}"))
            .execute(&mut observer)
            .await
            .unwrap();
        assert!(
            matches!(
                tx.table_meta("shape_later").await,
                Err(DbError::Unsupported {
                    feature: "table definition changed after transaction metadata snapshot",
                })
            ),
            "accepted shape change {change}"
        );
        assert!(matches!(
            tx.table_meta("shape_first").await,
            Err(DbError::Poisoned)
        ));
        tx.rollback().await.unwrap();
        let mut fresh = ds.begin().await.unwrap();
        let meta = fresh.table_meta("shape_later").await.unwrap();
        match change {
            "ADD COLUMN extra INT NULL" => assert_eq!(meta.columns().len(), 3),
            "MODIFY v INT NOT NULL FIRST" => assert_eq!(meta.columns()[0].name(), "v"),
            _ => assert!(meta.column("v").unwrap().nullable()),
        }
        fresh
            .execute(
                &SqlRecognizerFactory::recognize("UPDATE shape_later SET v=12 WHERE id=1").unwrap(),
                &[],
            )
            .await
            .unwrap();
        fresh.rollback().await.unwrap();
        let original: (i32, i32) = sqlx::query_as(
            "SELECT (SELECT v FROM shape_first WHERE id=1),v FROM shape_later WHERE id=1",
        )
        .fetch_one(&mut observer)
        .await
        .unwrap();
        assert_eq!(original, (10, 10));
    }
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn independent_extra_ddl_rejects_invisible_generated_and_auto_increment_columns() {
    use seata_at::SqlRecognizerFactory;

    let mut observer = admin().await;
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    for change in [
        "ADD COLUMN hidden_value INT NULL INVISIBLE",
        "MODIFY v INT GENERATED ALWAYS AS (id+1) STORED",
        "MODIFY id INT NOT NULL AUTO_INCREMENT",
    ] {
        sqlx::raw_sql(
            "DROP TABLE IF EXISTS extra_first,extra_later;
             CREATE TABLE extra_first(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             CREATE TABLE extra_later(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             INSERT INTO extra_first VALUES(1,10);
             INSERT INTO extra_later VALUES(1,10);",
        )
        .execute(&mut observer)
        .await
        .unwrap();
        let mut tx = ds.begin().await.unwrap();
        tx.execute(
            &SqlRecognizerFactory::recognize("UPDATE extra_first SET v=11 WHERE id=1").unwrap(),
            &[],
        )
        .await
        .unwrap();
        sqlx::query(&format!("ALTER TABLE extra_later {change}"))
            .execute(&mut observer)
            .await
            .unwrap();
        assert!(
            matches!(
                tx.table_meta("extra_later").await,
                Err(DbError::Unsupported { .. })
            ),
            "accepted independent attribute change {change}"
        );
        assert!(matches!(
            tx.table_meta("extra_first").await,
            Err(DbError::Poisoned)
        ));
        tx.rollback().await.unwrap();
        let original: i32 = sqlx::query_scalar("SELECT v FROM extra_first WHERE id=1")
            .fetch_one(&mut observer)
            .await
            .unwrap();
        assert_eq!(original, 10);
    }
    sqlx::raw_sql(
        "DROP TABLE IF EXISTS extra_first,extra_later;
         CREATE TABLE extra_first(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
         CREATE TABLE extra_later(id INT PRIMARY KEY,v VARCHAR(8) CHARACTER SET utf8mb4 NOT NULL)
             ENGINE=InnoDB DEFAULT CHARSET=ascii;
         INSERT INTO extra_first VALUES(1,10);
         INSERT INTO extra_later VALUES(1,'original');",
    )
    .execute(&mut observer)
    .await
    .unwrap();
    let mut tx = ds.begin().await.unwrap();
    tx.execute(
        &SqlRecognizerFactory::recognize("UPDATE extra_first SET v=11 WHERE id=1").unwrap(),
        &[],
    )
    .await
    .unwrap();
    sqlx::query("ALTER TABLE extra_later MODIFY v VARCHAR(8) CHARACTER SET utf8mb4 GENERATED ALWAYS AS ('fixed') STORED NOT NULL")
        .execute(&mut observer).await.unwrap();
    assert!(matches!(
        tx.table_meta("extra_later").await,
        Err(DbError::Unsupported { .. })
    ));
    assert!(matches!(
        tx.table_meta("extra_first").await,
        Err(DbError::Poisoned)
    ));
    tx.rollback().await.unwrap();
    let original: i32 = sqlx::query_scalar("SELECT v FROM extra_first WHERE id=1")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert_eq!(original, 10);
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn later_trigger_and_foreign_keys_are_rejected_before_business_or_cascade_effects() {
    use seata_at::SqlRecognizerFactory;

    let mut observer = admin().await;
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    for (change, expected) in [
        ("CREATE TRIGGER late_trigger AFTER UPDATE ON constraint_later FOR EACH ROW INSERT INTO constraint_effect VALUES(NEW.id)", "table trigger"),
        ("ALTER TABLE constraint_later ADD CONSTRAINT late_outbound FOREIGN KEY(v) REFERENCES constraint_parent(id)", "inbound/outbound foreign key"),
        ("ALTER TABLE constraint_child ADD CONSTRAINT late_inbound FOREIGN KEY(v) REFERENCES constraint_later(id) ON DELETE CASCADE", "inbound/outbound foreign key"),
    ] {
        sqlx::raw_sql(
            "DROP TABLE IF EXISTS constraint_child,constraint_later,constraint_parent,constraint_first,constraint_effect;
             CREATE TABLE constraint_first(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             CREATE TABLE constraint_parent(id INT PRIMARY KEY) ENGINE=InnoDB;
             CREATE TABLE constraint_later(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             CREATE TABLE constraint_child(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             CREATE TABLE constraint_effect(id INT) ENGINE=InnoDB;
             INSERT INTO constraint_first VALUES(1,10);
             INSERT INTO constraint_parent VALUES(10);
             INSERT INTO constraint_later VALUES(1,10);
             INSERT INTO constraint_child VALUES(2,1);",
        ).execute(&mut observer).await.unwrap();
        let mut tx = ds.begin().await.unwrap();
        tx.execute(&SqlRecognizerFactory::recognize("UPDATE constraint_first SET v=11 WHERE id=1").unwrap(), &[])
            .await.unwrap();
        sqlx::raw_sql(change).execute(&mut observer).await.unwrap();
        assert!(matches!(tx.table_meta("constraint_later").await,
            Err(DbError::Unsupported { feature }) if feature == expected
        ),"accepted late constraint {change}");
        assert!(matches!(tx.execute(
            &SqlRecognizerFactory::recognize("DELETE FROM constraint_later WHERE id=1").unwrap(), &[]
        ).await,Err(DbError::Poisoned)));
        tx.rollback().await.unwrap();
        let values: (i32,i64,i64,i64) = sqlx::query_as(
            "SELECT (SELECT v FROM constraint_first WHERE id=1),
                    (SELECT COUNT(*) FROM constraint_later),
                    (SELECT COUNT(*) FROM constraint_child),
                    (SELECT COUNT(*) FROM constraint_effect)",
        ).fetch_one(&mut observer).await.unwrap();
        assert_eq!(values,(10,1,1,0));
    }
    ds.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn checked_metadata_blocks_later_trigger_and_foreign_key_ddl_until_rollback() {
    use seata_at::SqlRecognizerFactory;

    let mut observer = admin().await;
    let ds = MySqlDataSource::connect(options("root")).await.unwrap();
    for (change, expected) in [
        ("CREATE TRIGGER postcheck_trigger AFTER UPDATE ON postcheck_parent FOR EACH ROW INSERT INTO postcheck_effect VALUES(NEW.id)", "table trigger"),
        ("ALTER TABLE postcheck_parent ADD CONSTRAINT postcheck_outbound FOREIGN KEY(v) REFERENCES postcheck_reference(id)", "inbound/outbound foreign key"),
        ("ALTER TABLE postcheck_child ADD CONSTRAINT postcheck_inbound FOREIGN KEY(v) REFERENCES postcheck_parent(id) ON DELETE CASCADE", "inbound/outbound foreign key"),
    ] {
        sqlx::raw_sql(
            "DROP TABLE IF EXISTS postcheck_child,postcheck_parent,postcheck_reference,postcheck_effect;
             CREATE TABLE postcheck_reference(id INT PRIMARY KEY) ENGINE=InnoDB;
             CREATE TABLE postcheck_parent(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             CREATE TABLE postcheck_child(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB;
             CREATE TABLE postcheck_effect(id INT) ENGINE=InnoDB;
             INSERT INTO postcheck_reference VALUES(10);
             INSERT INTO postcheck_parent VALUES(1,10);
             INSERT INTO postcheck_child VALUES(2,1);",
        ).execute(&mut observer).await.unwrap();
        let mut tx = ds.begin().await.unwrap();
        tx.table_meta("postcheck_parent").await.unwrap();
        sqlx::query("SET SESSION lock_wait_timeout=1")
            .execute(&mut observer).await.unwrap();
        // 用服务器返回的实际锁等待超时证明 DDL 被阻塞，避免将调度延迟当作锁证据。
        let error = sqlx::raw_sql(change).execute(&mut observer).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap()
                .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>().unwrap().number(),
            1205,
            "post-check constraint did not wait for original transaction: {change}"
        );
        tx.execute(&SqlRecognizerFactory::recognize(
            "UPDATE postcheck_parent SET v=11 WHERE id=1"
        ).unwrap(), &[]).await.unwrap();
        tx.execute(&SqlRecognizerFactory::recognize(
            "DELETE FROM postcheck_parent WHERE id=1"
        ).unwrap(), &[]).await.unwrap();
        tx.rollback().await.unwrap();
        let values: (i32, i64, i64) = sqlx::query_as(
            "SELECT (SELECT v FROM postcheck_parent WHERE id=1),
                    (SELECT COUNT(*) FROM postcheck_child),
                    (SELECT COUNT(*) FROM postcheck_effect)",
        ).fetch_one(&mut observer).await.unwrap();
        assert_eq!(values, (10, 1, 0));
        // 同一 DDL 在原事务释放 MDL 后确实成功，排除语法或权限错误造成的假阻塞。
        sqlx::raw_sql(change).execute(&mut observer).await.unwrap();
        let mut fresh = ds.begin().await.unwrap();
        assert!(matches!(fresh.table_meta("postcheck_parent").await,
            Err(DbError::Unsupported { feature }) if feature == expected
        ));
        assert!(matches!(fresh.table_meta("postcheck_parent").await, Err(DbError::Poisoned)));
        fresh.rollback().await.unwrap();
    }
    ds.close().await.unwrap();
}
