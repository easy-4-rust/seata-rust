//! JSON 物理域、精确字符串和原 Java Undo 类型；对应 Java ColumnMeta/Field/TableRecords。
#![allow(clippy::unwrap_used)]
use seata_at::{JacksonUndoLogParser, MySqlJdbcProfile, SqlType, SqlUndoLog};
use seata_datasource::{
    ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, MySqlType, Row, TableMeta,
    TableRecords,
};

fn column(nullable: bool) -> ColumnMeta {
    ColumnMeta::new(
        "v".into(),
        ColumnType::Text,
        nullable,
        2,
        None,
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::Json)
    .unwrap()
    .with_mysql_type(MySqlType::Json)
    .unwrap()
}

#[test]
fn json_domain_preserves_server_text_and_distinguishes_database_null() {
    let c = column(true);
    let exact = r#"{"n":9007199254740993,"u":18446744073709551615,"a":[null,true,"雪😀"]}"#;
    assert_eq!(
        Field::new(&c, FieldValue::Text(exact.into()))
            .unwrap()
            .value(),
        &FieldValue::Text(exact.into())
    );
    assert_ne!(
        Field::new(&c, FieldValue::Null).unwrap(),
        Field::new(&c, FieldValue::Text("null".into())).unwrap()
    );
    assert!(Field::new(&column(false), FieldValue::Null).is_err());
    assert!(Field::new(&c, FieldValue::Binary(vec![1])).is_err());
    assert!(Field::new(&c, FieldValue::Text("x".repeat(1_048_577))).is_err());
    assert!(ColumnMeta::new(
        "v".into(),
        ColumnType::Text,
        false,
        1,
        Some(1),
        None,
        None,
        None
    )
    .unwrap()
    .with_domain(ColumnDomain::Json)
    .is_err());
    assert!(c.clone().with_mysql_type(MySqlType::LongText).is_err());
    assert!(ColumnMeta::new(
        "v".into(),
        ColumnType::Text,
        true,
        1,
        None,
        None,
        None,
        None
    )
    .unwrap()
    .with_mysql_type(MySqlType::Json)
    .is_err());
}

#[test]
fn json_undo_uses_original_jdbc_string_and_restores_exactly() {
    let id = ColumnMeta::new(
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
        bits: 32,
        unsigned: false,
    })
    .unwrap()
    .with_mysql_type(MySqlType::Int)
    .unwrap();
    let v = column(true);
    let table = TableMeta::new("json_work".into(), vec![id.clone(), v.clone()]).unwrap();
    for value in [
        FieldValue::Null,
        FieldValue::Text("null".into()),
        FieldValue::Text(r#"{"u":18446744073709551615}"#.into()),
    ] {
        let mut rows = TableRecords::new(table.clone());
        rows.add(
            Row::new(
                &table,
                vec![
                    Field::new(&id, FieldValue::Signed(1)).unwrap(),
                    Field::new(&v, value.clone()).unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap();
        let log = SqlUndoLog::new(SqlType::Update, rows.clone(), rows).unwrap();
        let parser = JacksonUndoLogParser::default();
        let bytes = parser
            .encode_prepared(
                "127.0.0.1:8091:42",
                42,
                std::slice::from_ref(&log),
                MySqlJdbcProfile::Connector8Default,
            )
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains("\"type\":-1"));
        assert!(!text.contains("java.lang.Double"));
        assert!(parsed.is_object());
        let doc = parser.decode(&bytes).unwrap().unwrap();
        let restored = parser
            .restore_prepared(
                &doc,
                "127.0.0.1:8091:42",
                42,
                std::slice::from_ref(&table),
                MySqlJdbcProfile::Connector8Default,
            )
            .unwrap();
        assert_eq!(restored, vec![log]);
    }
}
