//! 精确 DATE/TIME 配置、原 Undo 和锁键的可观察行为。
#![allow(clippy::unwrap_used)]
use seata_at::{
    BaseTransactionalExecutor, JacksonUndoLogParser, MySqlJdbcProfile, SqlType, SqlUndoLog,
};
use seata_datasource::{
    ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, InternalTime, LocalDate, MySqlType,
    Row, TableMeta, TableRecords,
};
fn table(fsp: u8) -> TableMeta {
    let date = ColumnMeta::new(
        "d".into(),
        ColumnType::LocalDate,
        false,
        1,
        Some(1),
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::LocalDate)
    .unwrap()
    .with_mysql_type(MySqlType::Date)
    .unwrap();
    let time = ColumnMeta::new(
        "t".into(),
        ColumnType::InternalTime,
        false,
        2,
        Some(2),
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::InternalTime {
        fractional_seconds: fsp,
    })
    .unwrap()
    .with_mysql_type(MySqlType::Time)
    .unwrap();
    TableMeta::new("exact_keys".into(), vec![date, time]).unwrap()
}
fn image(table: &TableMeta, date: &str, time: &str) -> TableRecords {
    let mut rows = TableRecords::new(table.clone());
    rows.add(
        Row::new(
            table,
            vec![
                Field::new(
                    &table.columns()[0],
                    FieldValue::LocalDate(LocalDate::parse_sql(date).unwrap()),
                )
                .unwrap(),
                Field::new(
                    &table.columns()[1],
                    FieldValue::InternalTime(InternalTime::parse_sql(time).unwrap()),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    rows
}
#[test]
fn exact_profiles_keep_old_storage_codes_and_require_explicit_selection() {
    for code in 1..=6 {
        let p = MySqlJdbcProfile::from_storage_code(code).unwrap();
        assert_eq!(p.storage_code(), code);
        assert_eq!(p.exact_date_time(), code >= 5);
        assert_eq!(p.utc_temporal(), code >= 3);
        assert_eq!(p.boolean_tinyint(), code % 2 == 1);
    }
    assert!(MySqlJdbcProfile::from_storage_code(7).is_none());
    let table = table(6);
    let rows = image(&table, "1582-10-10", "-00:00:00.000001");
    let log = SqlUndoLog::new(SqlType::Update, rows.clone(), rows.clone()).unwrap();
    for code in 1..=4 {
        let p = MySqlJdbcProfile::from_storage_code(code).unwrap();
        assert!(JacksonUndoLogParser::default()
            .encode_prepared("xid", 42, std::slice::from_ref(&log), p)
            .is_err());
        assert!(BaseTransactionalExecutor::build_lock_key(&rows, p).is_err());
    }
}
#[test]
fn original_jackson_string_payload_restores_with_physical_identity() {
    let parser = JacksonUndoLogParser::default();
    let table = table(6);
    for code in [5, 6] {
        let p = MySqlJdbcProfile::from_storage_code(code).unwrap();
        for (date, time) in [
            ("1000-01-01", "-838:59:59"),
            ("1582-10-05", "-00:00:00.000001"),
            ("9999-12-31", "838:59:59"),
        ] {
            let rows = image(&table, date, time);
            let log = SqlUndoLog::new(SqlType::Update, rows.clone(), rows).unwrap();
            let bytes = parser
                .encode_prepared("xid", 42, std::slice::from_ref(&log), p)
                .unwrap();
            let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let fields = &raw["sqlUndoLogs"][1][0]["beforeImage"]["rows"][1][0]["fields"][1];
            assert_eq!(fields[0]["type"], 91);
            assert_eq!(fields[0]["value"], date);
            assert_eq!(fields[1]["type"], 92);
            assert_eq!(
                fields[1]["value"],
                InternalTime::parse_sql(time)
                    .unwrap()
                    .to_sql_string(6)
                    .unwrap()
            );
            let doc = parser.decode(&bytes).unwrap().unwrap();
            assert_eq!(
                parser
                    .restore_prepared(&doc, "xid", 42, std::slice::from_ref(&table), p)
                    .unwrap(),
                vec![log]
            );
            for replacement in [
                serde_json::json!("-00:00:00.0000001"),
                serde_json::json!(["java.sql.Time", "00:00:00"]),
                serde_json::json!("838:59:59.000001"),
                serde_json::json!("-00:00:00.00001"),
            ] {
                let mut bad = raw.clone();
                bad["sqlUndoLogs"][1][0]["beforeImage"]["rows"][1][0]["fields"][1][1]["value"] =
                    replacement;
                if let Ok(Some(doc)) = parser.decode(&serde_json::to_vec(&bad).unwrap()) {
                    assert!(parser
                        .restore_prepared(&doc, "xid", 42, std::slice::from_ref(&table), p)
                        .is_err());
                }
            }
        }
    }
}
#[test]
fn physical_time_precision_controls_the_java_string_lock_key() {
    for fsp in 0..=6 {
        let table = table(fsp);
        let rows = image(&table, "1582-10-14", "-25:01:02");
        let expected = InternalTime::parse_sql("-25:01:02")
            .unwrap()
            .to_sql_string(fsp)
            .unwrap();
        for code in [5, 6] {
            assert_eq!(
                BaseTransactionalExecutor::build_lock_key(
                    &rows,
                    MySqlJdbcProfile::from_storage_code(code).unwrap()
                )
                .unwrap(),
                Some(format!("exact_keys:1582-10-14_{expected}"))
            );
        }
        if fsp < 6 {
            assert!(Field::new(
                &table.columns()[1],
                FieldValue::InternalTime(InternalTime::from_microseconds(-1).unwrap())
            )
            .is_err());
        }
        assert!(table.columns()[0]
            .clone()
            .with_mysql_type(MySqlType::Time)
            .is_err());
    }
}

#[test]
fn null_temporal_columns_still_require_the_exact_profile() {
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
    let date = ColumnMeta::new(
        "d".into(),
        ColumnType::LocalDate,
        true,
        2,
        None,
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::LocalDate)
    .unwrap()
    .with_mysql_type(MySqlType::Date)
    .unwrap();
    let time = ColumnMeta::new(
        "t".into(),
        ColumnType::InternalTime,
        true,
        3,
        None,
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::InternalTime {
        fractional_seconds: 6,
    })
    .unwrap()
    .with_mysql_type(MySqlType::Time)
    .unwrap();
    let table = TableMeta::new("nullable_temporal".into(), vec![id, date, time]).unwrap();
    let mut rows = TableRecords::new(table.clone());
    rows.add(
        Row::new(
            &table,
            vec![
                Field::new(&table.columns()[0], FieldValue::Signed(1)).unwrap(),
                Field::new(&table.columns()[1], FieldValue::Null).unwrap(),
                Field::new(&table.columns()[2], FieldValue::Null).unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let log = SqlUndoLog::new(SqlType::Update, rows.clone(), rows).unwrap();
    let parser = JacksonUndoLogParser::default();
    for code in 1..=6 {
        let p = MySqlJdbcProfile::from_storage_code(code).unwrap();
        let encoded = parser.encode_prepared("xid", 42, std::slice::from_ref(&log), p);
        if code < 5 {
            assert!(encoded.is_err());
        } else {
            let bytes = encoded.unwrap();
            let doc = parser.decode(&bytes).unwrap().unwrap();
            assert_eq!(
                parser
                    .restore_prepared(&doc, "xid", 42, std::slice::from_ref(&table), p)
                    .unwrap(),
                vec![log.clone()]
            );
            for old in 1..=4 {
                assert!(parser
                    .restore_prepared(
                        &doc,
                        "xid",
                        42,
                        std::slice::from_ref(&table),
                        MySqlJdbcProfile::from_storage_code(old).unwrap()
                    )
                    .is_err());
            }
        }
    }
}
