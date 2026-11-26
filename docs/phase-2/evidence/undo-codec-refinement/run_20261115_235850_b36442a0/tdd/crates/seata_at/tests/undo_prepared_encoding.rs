//! 实际准备日志使用明确JDBC配置，不能猜测类包装或丢失物理整数。
#![allow(clippy::unwrap_used)]
use seata_at::{
    JacksonUndoLogParser, MySqlJdbcProfile, SqlType, SqlUndoLog, UndoCodecError, UndoCodecLimits,
};
use seata_datasource::{
    ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, MySqlType, Row, TableMeta,
    TableRecords,
};

fn column(name: &str, bits: u8, unsigned: bool, native: Option<MySqlType>, pk: bool) -> ColumnMeta {
    let column = ColumnMeta::new(
        name.into(),
        if unsigned {
            ColumnType::UnsignedInteger
        } else {
            ColumnType::SignedInteger
        },
        false,
        if pk { 1 } else { 2 },
        pk.then_some(1),
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::Integer { bits, unsigned })
    .unwrap();
    match native {
        Some(kind) => column.with_mysql_type(kind).unwrap(),
        None => column,
    }
}
fn inserted(value_column: ColumnMeta, value: FieldValue) -> SqlUndoLog {
    let pk = column("id", 32, false, Some(MySqlType::Int), true);
    let table = TableMeta::new("typed".into(), vec![pk, value_column]).unwrap();
    let mut rows = TableRecords::new(table.clone());
    rows.add(
        Row::new(
            &table,
            vec![
                Field::new(&table.columns()[0], FieldValue::Signed(1)).unwrap(),
                Field::new(&table.columns()[1], value).unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    SqlUndoLog::new(SqlType::Insert, TableRecords::new(table), rows).unwrap()
}

#[test]
fn prepared_node_preflight_accepts_the_exact_tree_quota_and_rejects_one_less() {
    let log = inserted(
        column("v", 32, false, Some(MySqlType::Int), false),
        FieldValue::Signed(2),
    );
    let profile = MySqlJdbcProfile::Connector8NumericTinyInt;
    let bytes = JacksonUndoLogParser::default()
        .encode_prepared("x", 1, std::slice::from_ref(&log), profile)
        .unwrap();
    fn nodes(value: &serde_json::Value) -> usize {
        1 + match value {
            serde_json::Value::Array(values) => values.iter().map(nodes).sum(),
            serde_json::Value::Object(values) => values.values().map(nodes).sum(),
            _ => 0,
        }
    }
    let count = nodes(&serde_json::from_slice(&bytes).unwrap());
    assert_eq!(count, 40);
    let exact = JacksonUndoLogParser::new(
        UndoCodecLimits::new(4096, count, 32, 128, 4096).unwrap(),
    );
    assert!(exact.decode(&bytes).unwrap().is_some());
    assert_eq!(
        exact.encode_prepared("x", 1, std::slice::from_ref(&log), profile).unwrap(),
        bytes
    );
    let smaller = JacksonUndoLogParser::new(
        UndoCodecLimits::new(4096, count - 1, 32, 128, 4096).unwrap(),
    );
    assert!(matches!(smaller.decode(&bytes), Err(UndoCodecError::Budget { .. })));
    assert!(matches!(
        smaller.encode_prepared("x", 1, &[log], profile),
        Err(UndoCodecError::Budget { .. })
    ));
}
#[test]
fn unsigned_and_bigint_wrappers_follow_actual_connector8_types_even_for_small_values() {
    let parser = JacksonUndoLogParser::default();
    for (bits, unsigned, native, value, jdbc, tag) in [
        (
            8,
            false,
            MySqlType::TinyInt {
                display_width_one: false,
            },
            FieldValue::Signed(-128),
            -6,
            None,
        ),
        (
            16,
            false,
            MySqlType::SmallInt,
            FieldValue::Signed(-32768),
            5,
            None,
        ),
        (
            24,
            false,
            MySqlType::MediumInt,
            FieldValue::Signed(-8388608),
            4,
            None,
        ),
        (
            32,
            false,
            MySqlType::Int,
            FieldValue::Signed(i32::MIN.into()),
            4,
            None,
        ),
        (
            64,
            false,
            MySqlType::BigInt,
            FieldValue::Signed(i64::MIN),
            -5,
            Some("java.lang.Long"),
        ),
        (
            8,
            true,
            MySqlType::TinyInt {
                display_width_one: false,
            },
            FieldValue::Unsigned(255),
            -6,
            None,
        ),
        (
            16,
            true,
            MySqlType::SmallInt,
            FieldValue::Unsigned(65535),
            5,
            None,
        ),
        (
            24,
            true,
            MySqlType::MediumInt,
            FieldValue::Unsigned(16777215),
            4,
            None,
        ),
        (
            32,
            true,
            MySqlType::Int,
            FieldValue::Unsigned(0),
            4,
            Some("java.lang.Long"),
        ),
        (
            32,
            true,
            MySqlType::Int,
            FieldValue::Unsigned(u32::MAX.into()),
            4,
            Some("java.lang.Long"),
        ),
        (
            64,
            true,
            MySqlType::BigInt,
            FieldValue::Unsigned(u64::MAX),
            -5,
            Some("java.math.BigInteger"),
        ),
    ] {
        let log = inserted(column("v", bits, unsigned, Some(native), false), value);
        let bytes = parser
            .encode_prepared(
                "fixture:xid",
                7,
                &[log],
                MySqlJdbcProfile::Connector8NumericTinyInt,
            )
            .unwrap();
        let doc = parser.decode(&bytes).unwrap().unwrap();
        let logs = doc.sql_undo_logs().unwrap();
        let field = &logs[0]["afterImage"]["rows"][1][0]["fields"][1][1];
        assert_eq!(field["type"].as_i64(), Some(jdbc));
        if let Some(class) = tag {
            assert_eq!(field["value"][0].as_str(), Some(class));
        } else {
            assert!(field["value"].is_number());
        }
        assert_eq!(
            logs[0]["beforeImage"]["@class"],
            "org.apache.seata.rm.datasource.sql.struct.TableRecords$EmptyTableRecords"
        );
    }
}
#[test]
fn default_boolean_profile_does_not_silently_destroy_physical_integer_two() {
    let parser = JacksonUndoLogParser::default();
    let log = inserted(
        column(
            "v",
            8,
            false,
            Some(MySqlType::TinyInt {
                display_width_one: true,
            }),
            false,
        ),
        FieldValue::Signed(2),
    );
    assert!(matches!(
        parser.encode_prepared(
            "x",
            7,
            std::slice::from_ref(&log),
            MySqlJdbcProfile::Connector8Default
        ),
        Err(UndoCodecError::Unsupported { .. })
    ));
    let bytes = parser
        .encode_prepared("x", 7, &[log], MySqlJdbcProfile::Connector8NumericTinyInt)
        .unwrap();
    let doc = parser.decode(&bytes).unwrap().unwrap();
    assert_eq!(
        doc.sql_undo_logs().unwrap()[0]["afterImage"]["rows"][1][0]["fields"][1][1]["value"]
            .as_i64(),
        Some(2)
    );
    let log = inserted(
        column(
            "v",
            8,
            false,
            Some(MySqlType::TinyInt {
                display_width_one: true,
            }),
            false,
        ),
        FieldValue::Signed(1),
    );
    let bytes = parser
        .encode_prepared("x", 7, &[log], MySqlJdbcProfile::Connector8Default)
        .unwrap();
    let doc = parser.decode(&bytes).unwrap().unwrap();
    let field = &doc.sql_undo_logs().unwrap()[0]["afterImage"]["rows"][1][0]["fields"][1][1];
    assert_eq!(field["type"].as_i64(), Some(-7));
    assert_eq!(field["value"].as_bool(), Some(true));
}
#[test]
fn missing_native_identity_and_unregistered_branch_arguments_cannot_encode_prepared_effects() {
    let parser = JacksonUndoLogParser::default();
    let log = inserted(column("v", 32, false, None, false), FieldValue::Signed(1));
    assert!(matches!(
        parser.encode_prepared("x", 7, &[log], MySqlJdbcProfile::Connector8Default),
        Err(UndoCodecError::Unsupported { .. })
    ));
    for (xid, branch) in [("", 1), ("x", 0), ("x", -1)] {
        assert!(parser
            .encode_prepared(xid, branch, &[], MySqlJdbcProfile::Connector8Default)
            .is_err());
    }
}
