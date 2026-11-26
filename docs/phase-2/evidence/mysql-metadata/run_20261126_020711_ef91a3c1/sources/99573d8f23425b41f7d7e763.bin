//! SQL Server 子类型的原 Java 差分和 Rust 共享句柄验证。
use seata_sqlparser::{ColumnMeta, MetadataLocale, SqlServerTableMeta, TableMeta};
use std::{error::Error, sync::Arc};

#[test]
fn sql_server_metadata_matches_original_inherited_dispatch() -> Result<(), Box<dyn Error>> {
    let locale = MetadataLocale::new("zh");
    let base = TableMeta::new(locale.clone());
    let sql_server = SqlServerTableMeta::new(locale.clone());
    let peer = SqlServerTableMeta::new(locale);
    let initial = sql_server.is_table_identify_existence();
    let initial_hash = sql_server.java_hash_code()?;
    sql_server.set_table_identify_existence(true);
    let enabled = sql_server.is_table_identify_existence();
    let peer_flag = peer.is_table_identify_existence();
    let equality = format!(
        "equals\t{}\t{}\t{}\t{}\t{}\t{}",
        base.java_equals(Some(&sql_server))?,
        sql_server.java_equals(Some(&base))?,
        sql_server.java_equals(Some(&peer))?,
        peer.java_equals(Some(&sql_server))?,
        sql_server.java_equals(None)?,
        sql_server.java_equals(Some(&"other"))?
    );
    let hashes = format!(
        "hash\t{initial_hash}\t{}\t{}",
        sql_server.java_hash_code()?,
        base.java_hash_code()?
    );
    let view: &TableMeta = &sql_server;
    view.set_table_name(Some("events".into()))?;
    let column = Arc::new(ColumnMeta::default());
    sql_server
        .all_columns()
        .put(Some("ID".into()), Some(column.clone()))?;
    let inherited = format!(
        "inherited\t{}\t{}\t{}\t{}\t{}",
        sql_server.table_name()?.ok_or("missing table name")?,
        Arc::ptr_eq(
            &view
                .column_meta(Some(&"id".into()))?
                .ok_or("missing column")?,
            &column
        ),
        sql_server.java_equals(Some(&base))?,
        base.java_equals(Some(&sql_server))?,
        view.java_equals(Some(&sql_server))?
    );
    let alias = sql_server.clone();
    alias.set_table_identify_existence(false);
    let identity = format!(
        "identity\t{initial}\t{enabled}\t{peer_flag}\t{}",
        sql_server.is_table_identify_existence()
    );
    let actual = [identity, equality, hashes, inherited].join("\n") + "\n";
    let expected = include_str!("fixtures/sqlparser_core/sql-server-table-java.tsv");
    assert_eq!(actual, expected.replace("\r\n", "\n"));
    Ok(())
}
