//! DATETIME 精确日历、UTC Undo、持久配置与主键；对应 Java LocalDateTime/ColumnMeta/Field。
#![allow(clippy::unwrap_used)]
use seata_at::{
    BaseTransactionalExecutor, JacksonUndoLogParser, MySqlJdbcProfile, SqlType, SqlUndoLog,
};
use seata_datasource::{
    ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, LocalDateTime, MySqlType, Row,
    TableMeta, TableRecords,
};

fn column(fsp: u8) -> ColumnMeta {
    ColumnMeta::new(
        "k".into(),
        ColumnType::LocalDateTime,
        false,
        1,
        Some(1),
        None,
        None,
        None,
    )
    .unwrap()
    .with_domain(ColumnDomain::LocalDateTime {
        fractional_seconds: fsp,
    })
    .unwrap()
    .with_mysql_type(MySqlType::DateTime)
    .unwrap()
}

#[test]
fn every_supported_gregorian_day_round_trips_without_a_host_timezone() {
    let mut days = 0;
    for year in 1000..=9999 {
        for month in 1..=12 {
            for day in 1..=31 {
                if let Ok(value) = LocalDateTime::new(year, month, day, 23, 59, 59, 999999000) {
                    assert_eq!(
                        LocalDateTime::from_epoch_seconds(
                            value.epoch_seconds(),
                            value.nanosecond()
                        )
                        .unwrap(),
                        value
                    );
                    days += 1;
                }
            }
        }
    }
    assert_eq!(days, 3_287_182);
    assert_eq!(
        LocalDateTime::parse_sql("1970-01-01 00:00:00")
            .unwrap()
            .epoch_seconds(),
        0
    );
    assert_eq!(
        LocalDateTime::parse_sql("1969-12-31 23:59:59.999999")
            .unwrap()
            .epoch_seconds(),
        -1
    );
}

#[test]
fn calendar_precision_and_profile_boundaries_are_not_guessed() {
    for invalid in [
        "0000-00-00 00:00:00",
        "0999-12-31 23:59:59",
        "1900-02-29 00:00:00",
        "2000-02-30 00:00:00",
        "2026-01-01T00:00:00",
        "2026-01-01 00:00:60",
        "2026-01-01 00:00:00.1234567",
        "2026-01-01 00:00:00.",
        "2026-01-01 00:00:00 ",
    ] {
        assert!(LocalDateTime::parse_sql(invalid).is_err());
    }
    assert!(LocalDateTime::from_epoch_seconds(i64::MIN, 0).is_err());
    assert!(LocalDateTime::from_epoch_seconds(i64::MAX, 0).is_err());
    assert!(LocalDateTime::new(2026, 1, 1, 0, 0, 0, 1).is_err());
    for fsp in 0..=6 {
        let c = column(fsp);
        let unit = 10u32.pow(9 - u32::from(fsp));
        let ns = if fsp == 0 { 0 } else { unit };
        assert!(Field::new(
            &c,
            FieldValue::LocalDateTime(LocalDateTime::new(2026, 1, 1, 0, 0, 0, ns).unwrap())
        )
        .is_ok());
        if fsp < 6 {
            assert!(Field::new(
                &c,
                FieldValue::LocalDateTime(LocalDateTime::new(2026, 1, 1, 0, 0, 0, 1000).unwrap())
            )
            .is_err());
        }
    }
    for (code, p) in [
        (1, MySqlJdbcProfile::Connector8Default),
        (2, MySqlJdbcProfile::Connector8NumericTinyInt),
        (3, MySqlJdbcProfile::Connector8DefaultUtc),
        (4, MySqlJdbcProfile::Connector8NumericTinyIntUtc),
    ] {
        assert_eq!(p.storage_code(), code);
        assert_eq!(MySqlJdbcProfile::from_storage_code(code), Some(p));
        assert_eq!(p.utc_temporal(), code >= 3);
        assert_eq!(p.boolean_tinyint(), code == 1 || code == 3);
    }
    assert_eq!(MySqlJdbcProfile::from_storage_code(0), None);
}

#[test]
fn utc_undo_preserves_negative_epoch_and_microseconds_and_refuses_old_profiles() {
    let c = column(6);
    let table = TableMeta::new("datetime_keys".into(), vec![c.clone()]).unwrap();
    let parser = JacksonUndoLogParser::default();
    for s in [
        "1000-01-01 00:00:00.000001",
        "1582-10-04 23:59:59.999999",
        "1969-12-31 23:59:59.999999",
        "1970-01-01 00:00:00",
        "9999-12-31 23:59:59.999999",
    ] {
        let value = LocalDateTime::parse_sql(s).unwrap();
        let mut rows = TableRecords::new(table.clone());
        rows.add(
            Row::new(
                &table,
                vec![Field::new(&c, FieldValue::LocalDateTime(value)).unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
        let log = SqlUndoLog::new(SqlType::Update, rows.clone(), rows).unwrap();
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
            assert!(String::from_utf8_lossy(&raw).contains("java.time.LocalDateTime"));
            let doc = parser.decode(&raw).unwrap().unwrap();
            assert_eq!(
                parser
                    .restore_prepared(&doc, "xid", 42, std::slice::from_ref(&table), p)
                    .unwrap(),
                vec![log.clone()]
            );
        }
    }
}

#[test]
fn datetime_primary_key_uses_java_seconds_and_fraction_rules() {
    let c = column(6);
    let table = TableMeta::new("datetime_keys".into(), vec![c.clone()]).unwrap();
    for (sql, java) in [
        ("2026-11-28 01:02:00", "2026-11-28T01:02"),
        ("2026-11-28 01:02:03.1", "2026-11-28T01:02:03.100"),
        ("2026-11-28 01:02:03.000001", "2026-11-28T01:02:03.000001"),
    ] {
        let value = LocalDateTime::parse_sql(sql).unwrap();
        assert_eq!(value.to_java_string(), java);
        let mut rows = TableRecords::new(table.clone());
        rows.add(
            Row::new(
                &table,
                vec![Field::new(&c, FieldValue::LocalDateTime(value)).unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            BaseTransactionalExecutor::build_lock_key(
                &rows,
                MySqlJdbcProfile::Connector8DefaultUtc
            )
            .unwrap(),
            Some(format!("datetime_keys:{java}"))
        );
    }
}
