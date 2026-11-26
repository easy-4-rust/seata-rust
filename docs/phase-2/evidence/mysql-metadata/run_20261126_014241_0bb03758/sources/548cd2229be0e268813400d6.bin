//! 全部当前支持物理类型在真实元数据中的身份，不以Rust值类别推测JDBC身份。
#![allow(clippy::unwrap_used, clippy::expect_used)]
use seata_datasource::{ColumnDomain, MySqlType};
use seata_db_mysql::MySqlDataSource;
#[path = "support/owned_mysql.rs"]
mod owned_mysql;

#[tokio::test]
#[ignore = "requires owned real MySQL fixture"]
async fn physical_types_keep_all_native_subtypes_and_tinyint_display_width() {
    let ds = MySqlDataSource::connect(owned_mysql::options(None))
        .await
        .unwrap();
    let mut tx = ds.begin().await.unwrap();
    let table = tx.table_meta("mysql_native_identity").await.unwrap();
    let expected = [
        ("id", MySqlType::Int),
        (
            "t8",
            MySqlType::TinyInt {
                display_width_one: false,
            },
        ),
        (
            "t1",
            MySqlType::TinyInt {
                display_width_one: true,
            },
        ),
        (
            "u8",
            MySqlType::TinyInt {
                display_width_one: false,
            },
        ),
        ("s16", MySqlType::SmallInt),
        ("m24", MySqlType::MediumInt),
        ("b64", MySqlType::BigInt),
        ("d", MySqlType::Decimal),
        ("c", MySqlType::Char),
        ("v", MySqlType::VarChar),
        ("tt", MySqlType::TinyText),
        ("t", MySqlType::Text),
        ("mt", MySqlType::MediumText),
        ("lt", MySqlType::LongText),
        ("b", MySqlType::Binary),
        ("vb", MySqlType::VarBinary),
        ("tb", MySqlType::TinyBlob),
        ("bl", MySqlType::Blob),
        ("mb", MySqlType::MediumBlob),
        ("lb", MySqlType::LongBlob),
    ];
    assert_eq!(table.columns().len(), expected.len());
    for (name, kind) in expected {
        assert_eq!(
            table.column(name).unwrap().mysql_type(),
            Some(kind),
            "native identity lost: {name}"
        );
    }
    assert_eq!(
        table.column("u8").unwrap().domain(),
        Some(&ColumnDomain::Integer {
            bits: 8,
            unsigned: true
        })
    );
    let declaration:String=sqlx::query_scalar("SELECT CAST(COLUMN_TYPE AS CHAR CHARACTER SET utf8mb4) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='mysql_native_identity' AND COLUMN_NAME='t1'")
        .fetch_one(&mut owned_mysql::admin().await).await.unwrap();
    assert_eq!(declaration, "tinyint(1)", "independent native declaration");
    tx.rollback().await.unwrap();
    ds.close().await.unwrap();
}
