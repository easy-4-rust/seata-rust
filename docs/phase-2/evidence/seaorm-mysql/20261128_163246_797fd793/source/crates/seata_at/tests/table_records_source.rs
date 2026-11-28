//! 固定 TableRecords 的五个原用例与 JDBC mock 投影对照。
use seata_at::{
    FastjsonUndoFieldValue as Value, JacksonUndoFieldValue as Wrapped, JacksonUndoRecords,
    TableRecordsError,
};
use seata_sqlparser::{r#struct::IndexType, ColumnMeta, IndexMeta, MetadataLocale, TableMeta};
use std::{error::Error, sync::Arc};

const LABELS: &[&str] = &["id", "name", "information", "description"];

fn meta() -> Result<TableMeta, Box<dyn Error>> {
    let table = TableMeta::new(MetadataLocale::new("en"));
    table.set_table_name(Some("table_records_test".into()))?;
    for (name, ty) in LABELS.iter().zip([4, 12, 2004, 2005]) {
        let column = ColumnMeta::default();
        column.set_column_name(Some((*name).into()))?;
        column.set_data_type(ty)?;
        table
            .all_columns()
            .put(Some((*name).into()), Some(Arc::new(column.clone())))?;
        if *name == "id" {
            let index = IndexMeta::default();
            index.set_indextype(Some(IndexType::PRIMARY))?;
            index.values()?.ok_or("no index list")?.push(Some(column))?;
            table
                .all_indexes()
                .put(Some("PRIMARY".into()), Some(Arc::new(index)))?;
        }
    }
    Ok(table)
}

fn original_rows() -> impl Iterator<Item = Vec<Wrapped>> {
    [(1, "Tom"), (2, "Jack")].into_iter().map(|(id, name)| {
        // 固定 Druid mock 的专用 Blob/Clob getter 返回存在且长度为零的值。
        [
            Value::Integer(id),
            Value::Text(name.into()),
            Value::Binary(Vec::new()),
            Value::Text(String::new()),
        ]
        .into_iter()
        .map(Wrapped::Common)
        .collect()
    })
}

#[test]
fn original_test_table_records() -> Result<(), Box<dyn Error>> {
    let mut records = JacksonUndoRecords::new(None, Vec::new());
    records.set_table_meta(TableMeta::new(MetadataLocale::new("en")))?;
    assert!(matches!(
        records.set_table_meta(TableMeta::new(MetadataLocale::new("en"))),
        Err(TableRecordsError::MetadataAlreadySet)
    ));
    assert_eq!(records.size(), 0);
    Ok(())
}

#[test]
fn original_test_pk_row() -> Result<(), Box<dyn Error>> {
    let records = JacksonUndoRecords::build_records(meta()?, LABELS, original_rows())?;
    let pk = records.pk_rows()?;
    assert_eq!(pk.len(), 2);
    let mut actual = Vec::new();
    for row in records.rows() {
        for field in row {
            let (class, value) = match field.value() {
                Wrapped::Common(Value::Integer(n)) => ("Integer", n.to_string()),
                Wrapped::Common(Value::Text(s)) => ("String", s.clone()),
                Wrapped::SerialBlob(b) if b.is_empty() => ("SerialBlob", String::new()),
                Wrapped::SerialClob(s) => ("SerialClob", s.clone()),
                _ => return Err("unexpected JDBC field value".into()),
            };
            let key = if field.is_primary_key() {
                "PRIMARY_KEY"
            } else {
                "NULL"
            };
            actual.push(format!(
                "SEATA_RECORD_FIELD\t{}\t{}\t{key}\t{class}\t{value}",
                field.name(),
                field.jdbc_type()
            ));
        }
    }
    for row in pk {
        match row.get("id").map(|field| field.value()) {
            Some(Wrapped::Common(Value::Integer(n))) => {
                actual.push(format!("SEATA_RECORD_PK\t{}\t{n}", row.len()))
            }
            _ => return Err("missing original primary key".into()),
        }
    }
    assert_eq!(
        actual,
        include_str!("fixtures/table_records_java.tsv")
            .lines()
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn original_test_build_records() -> Result<(), Box<dyn Error>> {
    let records = JacksonUndoRecords::build_records(meta()?, LABELS, original_rows())?;
    assert_eq!(records.size(), 2);
    assert_eq!(records.table_name(), Some("table_records_test"));
    Ok(())
}

#[test]
fn original_test_build_records_new_field() -> Result<(), Box<dyn Error>> {
    let labels = ["id", "name", "information", "description", "newf"];
    let rows = original_rows().map(|mut row| {
        row.push(Wrapped::Common(Value::Text("newf".into())));
        row
    });
    assert!(matches!(
        JacksonUndoRecords::build_records(meta()?, &labels, rows),
        Err(TableRecordsError::UnknownColumn)
    ));
    Ok(())
}

#[test]
fn original_test_empty() -> Result<(), Box<dyn Error>> {
    assert_eq!(JacksonUndoRecords::empty(None).size(), 0);
    let mut empty = JacksonUndoRecords::empty(None);
    empty.set_table_meta(TableMeta::new(MetadataLocale::new("en")))?;
    assert_eq!(empty.size(), 0);
    assert!(empty.rows().is_empty());
    assert!(empty.pk_rows()?.is_empty());
    assert!(matches!(
        empty.add(Vec::new()),
        Err(TableRecordsError::EmptyRecords)
    ));
    assert!(matches!(
        empty.table_meta(),
        Err(TableRecordsError::EmptyRecords)
    ));
    Ok(())
}

#[test]
fn original_test_build_records_with_offset_date_time() -> Result<(), Box<dyn Error>> {
    use seata_at::OffsetDateTime;

    let meta = meta()?;
    let time = ColumnMeta::default();
    time.set_column_name(Some("time_col".into()))?;
    time.set_data_type(2014)?;
    meta.all_columns()
        .put(Some("time_col".into()), Some(Arc::new(time)))?;

    // 原输入 2025-01-15T10:30:45Z，从 1970 年起计算到当日的 UTC 秒数。
    let days = 55 * 365 + 14 + 14;
    let original = OffsetDateTime::from_epoch(days * 86400 + 10 * 3600 + 30 * 60 + 45, 0, 0)?;
    let records = JacksonUndoRecords::build_records(
        meta,
        &["id", "time_col"],
        [vec![
            Wrapped::Common(Value::Integer(1)),
            Wrapped::OffsetDateTime(original),
        ]],
    )?;
    assert_eq!(records.size(), 1);
    let field = records.rows()[0]
        .iter()
        .find(|f| f.name().eq_ignore_ascii_case("time_col"))
        .ok_or("missing time_col")?;
    assert_eq!(field.jdbc_type(), 2014);
    let Wrapped::OffsetDateTime(value) = field.value() else {
        return Err("missing OffsetDateTime JDBC object".into());
    };
    assert_eq!(field.value().java_class(), "java.time.OffsetDateTime");
    let observation = include_str!("fixtures/table_records_offset_java.tsv");
    let java_wire = observation
        .trim_end()
        .rsplit('\t')
        .next()
        .ok_or("missing Java timestamp")?;
    let encoded = value.encode_jackson_timestamp();
    assert_eq!(encoded, java_wire);
    assert_eq!(
        OffsetDateTime::decode_jackson_timestamp(encoded.as_bytes())?,
        original
    );
    Ok(())
}

#[test]
fn jackson_offset_timestamp_matches_java_boundaries() -> Result<(), Box<dyn Error>> {
    use seata_at::OffsetDateTime;

    let observed = include_str!("fixtures/offset_datetime_boundaries_java.tsv");
    for line in observed.lines() {
        let columns: Vec<_> = line.split('\t').collect();
        match columns.as_slice() {
            ["TIME", _source, sec, nanos, offset, json, back_sec, back_nanos, back_offset] => {
                let source =
                    OffsetDateTime::from_epoch(sec.parse()?, nanos.parse()?, offset.parse()?)?;
                assert_eq!(source.encode_jackson_timestamp(), *json);
                assert_eq!(
                    OffsetDateTime::decode_jackson_timestamp(json.as_bytes())?,
                    OffsetDateTime::from_epoch(
                        back_sec.parse()?,
                        back_nanos.parse()?,
                        back_offset.parse()?
                    )?
                );
            }
            ["READ", json, sec, nanos] => {
                assert_eq!(
                    OffsetDateTime::decode_jackson_timestamp(json.as_bytes())?,
                    OffsetDateTime::from_epoch(sec.parse()?, nanos.parse()?, 0)?
                );
            }
            ["REJECT", json, "NumberFormatException"] => {
                assert!(OffsetDateTime::decode_jackson_timestamp(json.as_bytes()).is_err());
            }
            ["LIMITS", min, max] => {
                let (min, max) = (min.parse::<i64>()?, max.parse::<i64>()?);
                assert!(OffsetDateTime::from_epoch(min - 1, 0, 0).is_err());
                assert!(OffsetDateTime::from_epoch(max + 1, 0, 0).is_err());
                assert!(OffsetDateTime::from_epoch(max + 64800, 0, -64800).is_ok());
            }
            ["UNDO", class, wire] => {
                let parser = seata_at::JacksonUndoLogParser::default();
                let document = parser
                    .decode(wire.as_bytes())?
                    .ok_or("missing Java branch")?;
                let logs = parser
                    .decode_typed_logs(&document)?
                    .ok_or("missing Java logs")?;
                let field = &logs[0].after_image().ok_or("missing after image")?.rows()[0][1];
                assert_eq!(field.value().java_class(), *class);
                assert_eq!(
                    parser.encode_typed_branch(
                        document.xid(),
                        document.branch_id(),
                        Some(&logs)
                    )?,
                    wire.as_bytes()
                );
            }
            _ => return Err("invalid pinned Java observation".into()),
        }
    }
    for bad in [
        b"01".as_slice(),
        b"1e9999",
        b"\"2025-01-15T10:30:45Z\"",
        b"[]",
        b"1 2",
    ] {
        assert!(OffsetDateTime::decode_jackson_timestamp(bad).is_err());
    }
    assert!(OffsetDateTime::from_epoch(0, 1_000_000_000, 0).is_err());
    assert!(OffsetDateTime::from_epoch(0, 0, 64801).is_err());
    Ok(())
}
