//! 原锁键字节与主键顺序，不把纯格式结果当成TC锁覆盖证明。
#![allow(clippy::unwrap_used)]
use seata_at::{
    BaseTransactionalExecutor, ConnectionContext, MySqlJdbcProfile, SqlType, SqlUndoLog,
};
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, MySqlType, Row,
    TableMeta, TableRecords,
};

const PROFILE: MySqlJdbcProfile = MySqlJdbcProfile::Connector8NumericTinyInt;
fn text_table(name: &str, composite: bool) -> TableMeta {
    let mut columns = vec![ColumnMeta::new(
        "id".into(),
        ColumnType::Text,
        false,
        1,
        Some(1),
        Some(128),
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::Text {
        charset: CharacterSet::Utf8Mb4,
        character_limit: Some(32),
        fixed_width: false,
    })
    .unwrap()
    .with_mysql_type(MySqlType::VarChar)
    .unwrap()];
    if composite {
        columns.push(
            ColumnMeta::new(
                "userId".into(),
                ColumnType::Text,
                false,
                2,
                Some(2),
                Some(128),
                None,
                None,
            )
            .unwrap()
            .with_domain(ColumnDomain::Text {
                charset: CharacterSet::Utf8Mb4,
                character_limit: Some(32),
                fixed_width: false,
            })
            .unwrap()
            .with_mysql_type(MySqlType::VarChar)
            .unwrap(),
        );
    }
    TableMeta::new(name.into(), columns).unwrap()
}
fn records(table: &TableMeta, values: &[&[&str]]) -> TableRecords {
    let mut records = TableRecords::new(table.clone());
    for values in values {
        records
            .add(
                Row::new(
                    table,
                    table
                        .columns()
                        .iter()
                        .zip(*values)
                        .map(|(column, value)| {
                            Field::new(column, FieldValue::Text((*value).into())).unwrap()
                        })
                        .collect(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    records
}
#[test]
fn original_single_and_composite_pk_lock_bytes_preserve_row_order() {
    let single = text_table("test_name", false);
    let rows = records(&single, &[&["1"], &["2"]]);
    assert_eq!(
        BaseTransactionalExecutor::build_lock_key(&rows, PROFILE)
            .unwrap()
            .as_deref(),
        Some("test_name:1,2")
    );
    let composite = text_table("test_name", true);
    let rows = records(&composite, &[&["1", "one"], &["2", "two"]]);
    assert_eq!(
        BaseTransactionalExecutor::build_lock_key(&rows, PROFILE)
            .unwrap()
            .as_deref(),
        Some("test_name:1_one,2_two")
    );
    let reversed = records(&composite, &[&["2", "two"], &["1", "one"]]);
    assert_eq!(
        BaseTransactionalExecutor::build_lock_key(&reversed, PROFILE)
            .unwrap()
            .as_deref(),
        Some("test_name:2_two,1_one")
    );
}
#[test]
fn delete_uses_before_insert_and_update_use_after_and_full_chunks_are_deduplicated() {
    let table = text_table("test_name", false);
    let rows = records(&table, &[&["1"], &["2"]]);
    let insert = SqlUndoLog::new(
        SqlType::Insert,
        TableRecords::new(table.clone()),
        rows.clone(),
    )
    .unwrap();
    let delete = SqlUndoLog::new(
        SqlType::Delete,
        rows.clone(),
        TableRecords::new(table.clone()),
    )
    .unwrap();
    let update = SqlUndoLog::new(SqlType::Update, rows.clone(), rows).unwrap();
    let context = ConnectionContext::from_undo_logs(&[insert, delete, update], PROFILE).unwrap();
    assert_eq!(context.build_lock_keys().as_deref(), Some("test_name:1,2"));
    assert_eq!(context.lock_key_count(), 1);
    assert!(ConnectionContext::from_undo_logs(&[], PROFILE)
        .unwrap()
        .build_lock_keys()
        .is_none());
}
#[test]
fn original_text_comma_rejection_is_redacted_and_failed_append_is_atomic() {
    let table = text_table("private_key", false);
    let before = records(&table, &[&["allowed"]]);
    let valid = SqlUndoLog::new(SqlType::Insert, TableRecords::new(table.clone()), before).unwrap();
    let invalid = SqlUndoLog::new(
        SqlType::Insert,
        TableRecords::new(table.clone()),
        records(&table, &[&["secret,value"]]),
    )
    .unwrap();
    let mut context = ConnectionContext::from_undo_logs(&[valid], PROFILE).unwrap();
    let error = context.append_undo(&invalid, PROFILE).unwrap_err();
    assert!(!format!("{error:?} {error}").contains("secret"));
    assert_eq!(
        context.build_lock_keys().as_deref(),
        Some("private_key:allowed")
    );
    assert!(!format!("{context:?}").contains("allowed"));
}
