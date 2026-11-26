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
    for log in [&insert, &delete, &update] {
        assert_eq!(
            ConnectionContext::from_undo_logs(std::slice::from_ref(log), PROFILE)
                .unwrap()
                .build_lock_keys()
                .as_deref(),
            Some("test_name:1,2")
        );
    }
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

#[test]
fn one_transaction_cannot_mix_jdbc_profiles_even_when_the_current_key_bytes_match() {
    let table = text_table("fixed", false);
    let log = SqlUndoLog::new(
        SqlType::Insert,
        TableRecords::new(table.clone()),
        records(&table, &[&["key"]]),
    )
    .unwrap();
    let mut context =
        ConnectionContext::from_undo_logs(std::slice::from_ref(&log), PROFILE).unwrap();
    assert!(context
        .append_undo(&log, MySqlJdbcProfile::Connector8Default)
        .is_err());
    assert_eq!(context.build_lock_keys().as_deref(), Some("fixed:key"));
    let mut empty = ConnectionContext::from_undo_logs(&[], PROFILE).unwrap();
    assert!(empty
        .append_undo(&log, MySqlJdbcProfile::Connector8Default)
        .is_err());
    assert!(empty.build_lock_keys().is_none());
}

#[test]
fn aggregate_budget_and_log_count_are_atomic_and_include_separators() {
    let table = text_table("t", false);
    let first = SqlUndoLog::new(
        SqlType::Insert,
        TableRecords::new(table.clone()),
        records(&table, &[&["one"]]),
    )
    .unwrap();
    let second = SqlUndoLog::new(
        SqlType::Insert,
        TableRecords::new(table.clone()),
        records(&table, &[&["two"]]),
    )
    .unwrap();
    let mut exact = ConnectionContext::with_limit(11).unwrap();
    exact.append_undo(&second, PROFILE).unwrap();
    exact.append_undo(&first, PROFILE).unwrap();
    assert_eq!(exact.build_lock_keys().as_deref(), Some("t:one;t:two"));
    let mut too_small = ConnectionContext::with_limit(10).unwrap();
    too_small.append_undo(&first, PROFILE).unwrap();
    assert!(too_small.append_undo(&second, PROFILE).is_err());
    assert_eq!(too_small.build_lock_keys().as_deref(), Some("t:one"));
    let mut count = ConnectionContext::new();
    for _ in 0..128 {
        count.append_undo(&first, PROFILE).unwrap();
    }
    assert!(count.append_undo(&first, PROFILE).is_err());
    assert_eq!(count.lock_key_count(), 1);
    assert_eq!(count.build_lock_keys().as_deref(), Some("t:one"));
}

#[test]
fn single_key_budget_includes_table_prefix_and_expanded_signed_binary_bytes() {
    // 人工模型仅验证输出预算，不把超大VARCHAR主键当作已认证MySQL DDL。
    for binary in [false, true] {
        let column = ColumnMeta::new(
            "id".into(),
            if binary {
                ColumnType::Binary
            } else {
                ColumnType::Text
            },
            false,
            1,
            Some(1),
            Some(1_048_576),
            None,
            None,
        )
        .unwrap()
        .with_domain(if binary {
            ColumnDomain::Binary { fixed_width: false }
        } else {
            ColumnDomain::Text {
                charset: CharacterSet::Ascii,
                character_limit: Some(1_048_576),
                fixed_width: false,
            }
        })
        .unwrap()
        .with_mysql_type(if binary {
            MySqlType::VarBinary
        } else {
            MySqlType::VarChar
        })
        .unwrap();
        let table = TableMeta::new("t".into(), vec![column]).unwrap();
        let make = |value| {
            let mut records = TableRecords::new(table.clone());
            records
                .add(
                    Row::new(
                        &table,
                        vec![Field::new(&table.columns()[0], value).unwrap()],
                    )
                    .unwrap(),
                )
                .unwrap();
            records
        };
        if binary {
            let records = make(FieldValue::Binary(vec![128; 200_000]));
            assert!(BaseTransactionalExecutor::build_lock_key(&records, PROFILE).is_err());
        } else {
            let exact = make(FieldValue::Text("x".repeat(1_048_574)));
            assert_eq!(
                BaseTransactionalExecutor::build_lock_key(&exact, PROFILE)
                    .unwrap()
                    .unwrap()
                    .len(),
                1_048_576
            );
            let over = make(FieldValue::Text("x".repeat(1_048_575)));
            assert!(BaseTransactionalExecutor::build_lock_key(&over, PROFILE).is_err());
        }
    }
}

#[test]
fn default_tiny_boolean_does_not_destroy_integer_two_and_unknown_native_is_rejected() {
    let column = ColumnMeta::new(
        "id".into(),
        ColumnType::SignedInteger,
        false,
        1,
        Some(1),
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::Integer {
        bits: 8,
        unsigned: false,
    })
    .unwrap()
    .with_mysql_type(MySqlType::TinyInt {
        display_width_one: true,
    })
    .unwrap();
    let table = TableMeta::new("tiny".into(), vec![column]).unwrap();
    for (value, expected) in [(0, Some("tiny:false")), (1, Some("tiny:true")), (2, None)] {
        let mut records = TableRecords::new(table.clone());
        records
            .add(
                Row::new(
                    &table,
                    vec![Field::new(&table.columns()[0], FieldValue::Signed(value)).unwrap()],
                )
                .unwrap(),
            )
            .unwrap();
        let actual = BaseTransactionalExecutor::build_lock_key(
            &records,
            MySqlJdbcProfile::Connector8Default,
        );
        if let Some(expected) = expected {
            assert_eq!(actual.unwrap().as_deref(), Some(expected));
        } else {
            assert!(actual.is_err());
        }
        assert_eq!(
            BaseTransactionalExecutor::build_lock_key(&records, PROFILE)
                .unwrap()
                .unwrap(),
            format!("tiny:{value}")
        );
    }
    let legacy = ColumnMeta::new(
        "id".into(),
        ColumnType::SignedInteger,
        false,
        1,
        Some(1),
        None,
        None,
        None,
    )
    .unwrap();
    let table = TableMeta::new("legacy".into(), vec![legacy]).unwrap();
    let mut legacy_records = TableRecords::new(table.clone());
    legacy_records
        .add(
            Row::new(
                &table,
                vec![Field::new(&table.columns()[0], FieldValue::Signed(1)).unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
    let log = SqlUndoLog::new(SqlType::Insert, TableRecords::new(table), legacy_records).unwrap();
    let mut context = ConnectionContext::new();
    assert!(context
        .append_undo(&log, MySqlJdbcProfile::Connector8Default)
        .is_err());
    assert!(context.build_lock_keys().is_none());
    let table = text_table("accepted", false);
    let valid = SqlUndoLog::new(
        SqlType::Insert,
        TableRecords::new(table.clone()),
        records(&table, &[&["key"]]),
    )
    .unwrap();
    context.append_undo(&valid, PROFILE).unwrap();
    assert_eq!(context.build_lock_keys().as_deref(), Some("accepted:key"));
}
