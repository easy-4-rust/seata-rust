//! TIMESTAMP 的独立时间点、原 Timestamp 格式和主键；对应 Java java.sql.Timestamp。
#![allow(clippy::unwrap_used)]
use seata_at::{
    BaseTransactionalExecutor, JacksonUndoLogParser, MySqlJdbcProfile, SqlType, SqlUndoLog,
};
use seata_datasource::{
    ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, MySqlType, Row, TableMeta,
    TableRecords, Timestamp,
};

fn column(fsp: u8) -> ColumnMeta {
    ColumnMeta::new(
        "k".into(),
        ColumnType::Timestamp,
        false,
        1,
        Some(1),
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::Timestamp {
        fractional_seconds: fsp,
    })
    .unwrap()
    .with_mysql_type(MySqlType::Timestamp)
    .unwrap()
}

fn records(table: &TableMeta, value: Timestamp) -> TableRecords {
    let mut rows = TableRecords::new(table.clone());
    rows.add(
        Row::new(
            table,
            vec![Field::new(&table.columns()[0], FieldValue::Timestamp(value)).unwrap()],
        )
        .unwrap(),
    )
    .unwrap();
    rows
}

#[test]
fn timestamp_range_precision_and_native_identity_are_explicit() {
    for seconds in [i64::MIN, -1, 0, 2_147_483_648, i64::MAX] {
        assert!(Timestamp::from_epoch_seconds(seconds, 0).is_err());
    }
    for nanos in [1, 999, 1_000_000_000, u32::MAX] {
        assert!(Timestamp::from_epoch_seconds(1, nanos).is_err());
    }
    assert_eq!(
        Timestamp::from_epoch_seconds(1, 0)
            .unwrap()
            .to_sql_utc_string(),
        "1970-01-01 00:00:01"
    );
    assert_eq!(
        Timestamp::from_epoch_seconds(2_147_483_647, 999_999_000)
            .unwrap()
            .to_sql_utc_string(),
        "2038-01-19 03:14:07.999999"
    );
    assert!(Timestamp::parse_sql_utc("2001-02-30 12:00:00").is_err());
    for fsp in 0..=6 {
        let c = column(fsp);
        let nanos = if fsp == 0 {
            0
        } else {
            10u32.pow(9 - u32::from(fsp))
        };
        assert!(Field::new(
            &c,
            FieldValue::Timestamp(Timestamp::from_epoch_seconds(1, nanos).unwrap())
        )
        .is_ok());
        if fsp < 6 {
            assert!(Field::new(
                &c,
                FieldValue::Timestamp(Timestamp::from_epoch_seconds(1, 1000).unwrap())
            )
            .is_err());
        }
        assert!(c.clone().with_mysql_type(MySqlType::DateTime).is_err());
        assert!(c
            .with_domain(ColumnDomain::LocalDateTime {
                fractional_seconds: fsp
            })
            .is_err());
    }
}

#[test]
fn original_timestamp_scalar_and_nanos_remain_distinct_from_local_datetime() {
    let table = TableMeta::new("timestamp_keys".into(), vec![column(6)]).unwrap();
    let parser = JacksonUndoLogParser::default();
    for (seconds, nanos) in [
        (1, 0),
        (1, 1000),
        (951_827_696, 123_456_000),
        (2_147_483_647, 999_999_000),
    ] {
        let value = Timestamp::from_epoch_seconds(seconds, nanos).unwrap();
        let image = records(&table, value);
        let log = SqlUndoLog::new(SqlType::Update, image.clone(), image).unwrap();
        for p in [
            MySqlJdbcProfile::Connector8Default,
            MySqlJdbcProfile::Connector8NumericTinyInt,
        ] {
            assert!(parser
                .encode_prepared("xid", 42, std::slice::from_ref(&log), p)
                .is_err());
        }
        for p in [
            MySqlJdbcProfile::Connector8DefaultUtc,
            MySqlJdbcProfile::Connector8NumericTinyIntUtc,
        ] {
            let raw = parser
                .encode_prepared("xid", 42, std::slice::from_ref(&log), p)
                .unwrap();
            let original: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            let field = &original["sqlUndoLogs"][1][0]["beforeImage"]["rows"][1][0]["fields"][1][0];
            assert_eq!(field["type"], 93);
            assert_eq!(field["value"][0], "java.sql.Timestamp");
            let millis = seconds * 1000 + i64::from(nanos / 1_000_000);
            assert_eq!(
                field["value"][1],
                if nanos % 1_000_000 == 0 {
                    serde_json::json!(millis)
                } else {
                    serde_json::json!([millis, nanos])
                }
            );
            let doc = parser.decode(&raw).unwrap().unwrap();
            assert_eq!(
                parser
                    .restore_prepared(&doc, "xid", 42, std::slice::from_ref(&table), p)
                    .unwrap(),
                vec![log.clone()]
            );
            for replacement in [
                serde_json::json!(["java.time.LocalDateTime", [millis, nanos]]),
                serde_json::json!(["java.sql.Timestamp", [millis, 1]]),
                serde_json::json!(["java.sql.Timestamp", [millis, 1_000_000_000]]),
                serde_json::json!(["java.sql.Timestamp", [-1, nanos]]),
            ] {
                let mut altered = original.clone();
                altered["sqlUndoLogs"][1][0]["beforeImage"]["rows"][1][0]["fields"][1][0]
                    ["value"] = replacement;
                if let Ok(Some(doc)) = parser.decode(&serde_json::to_vec(&altered).unwrap()) {
                    assert!(parser
                        .restore_prepared(&doc, "xid", 42, std::slice::from_ref(&table), p)
                        .is_err());
                }
            }
        }
    }
}

#[test]
fn timestamp_lock_key_keeps_java_space_seconds_and_trimmed_fraction() {
    let table = TableMeta::new("timestamp_keys".into(), vec![column(6)]).unwrap();
    for (text, java) in [
        ("1970-01-01 00:00:01", "1970-01-01 00:00:01.0"),
        ("2000-02-29 12:34:56.1", "2000-02-29 12:34:56.1"),
        ("2038-01-19 03:14:07.000001", "2038-01-19 03:14:07.000001"),
    ] {
        let value = Timestamp::parse_sql_utc(text).unwrap();
        assert_eq!(value.to_java_utc_string(), java);
        let rows = records(&table, value);
        assert_eq!(
            BaseTransactionalExecutor::build_lock_key(
                &rows,
                MySqlJdbcProfile::Connector8DefaultUtc
            )
            .unwrap(),
            Some(format!("timestamp_keys:{java}"))
        );
    }
}
