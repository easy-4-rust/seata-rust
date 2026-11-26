//! 固定 Jackson 原默认值、H2 对象图和循环断言。
#![allow(clippy::unwrap_used)]
#[path = "support/undo_h2_input.rs"]
mod input;
use input::from_h2;
use seata_at::JacksonUndoLogParser;

#[test]
fn jackson_source_test_decode_default_content() {
    let parser = JacksonUndoLogParser::default();
    let wire = parser.default_content();
    let document = parser.decode(wire).unwrap();
    assert!(document.is_some());
    let document = document.unwrap();
    let null_xid = document.xid().is_none();
    let null_logs = document.sql_undo_logs().is_none();
    let id = document.branch_id();
    assert!(null_xid);
    assert_eq!(id, 0);
    assert!(null_logs);
    println!(
        "SEATA_FASTJSON_DEFAULT\t{}\ttrue\t{null_xid}\t{id}\t{null_logs}",
        std::str::from_utf8(wire).unwrap()
    );
}

#[test]
#[ignore = "requires fixed source JDBC H2 input trace"]
fn jackson_source_test_encode_and_decode() {
    let (xid, id, logs) = from_h2("testEncodeAndDecode");
    let parser = JacksonUndoLogParser::default();
    let wire = parser.encode_original(Some(&xid), id, Some(&logs)).unwrap();
    assert!(!wire.is_empty());
    let document = parser.decode(&wire).unwrap().unwrap();
    assert_eq!(document.branch_id(), id);
    assert_eq!(document.xid(), Some(xid.as_str()));
    let decoded = parser.decode_original_logs(&document).unwrap().unwrap();
    assert_eq!(decoded.len(), logs.len());
    let mut observed = vec![
        (!wire.is_empty()).to_string(),
        document.branch_id().to_string(),
        document.xid().unwrap().into(),
        decoded.len().to_string(),
    ];
    for (expected, actual) in logs.iter().zip(&decoded) {
        assert_eq!(expected.sql_type(), actual.sql_type());
        assert_eq!(expected.table_name(), actual.table_name());
        let before = expected.before_image() == actual.before_image();
        let after = expected.after_image() == actual.after_image();
        assert!(before && after);
        observed.extend([
            format!("{:?}", actual.sql_type()).to_ascii_uppercase(),
            actual.table_name().unwrap().into(),
            before.to_string(),
            after.to_string(),
        ]);
    }
    observed.push(String::from_utf8(wire).unwrap());
    println!("SEATA_FASTJSON_H2_ROUNDTRIP\t{}", observed.join("\t"));
}

#[test]
#[ignore = "requires fixed H2 trace and original 100000 codec iterations"]
fn jackson_source_test_performance() {
    let (xid, id, logs) = from_h2("testPerformance");
    let parser = JacksonUndoLogParser::default();
    let mut completed = 0;
    let mut wire = Vec::new();
    let mut observed_id = None;
    for _ in 0..100_000 {
        wire = parser.encode_original(Some(&xid), id, Some(&logs)).unwrap();
        assert!(!wire.is_empty());
        let document = parser.decode(&wire).unwrap().unwrap();
        observed_id = Some(document.branch_id());
        assert_eq!(observed_id, Some(id));
        std::hint::black_box(parser.decode_original_logs(&document).unwrap().unwrap());
        completed += 1;
    }
    println!(
        "SEATA_FASTJSON_H2_PERFORMANCE\t{completed}\t{}\t{}\t{}",
        !wire.is_empty(),
        observed_id.unwrap(),
        String::from_utf8(wire).unwrap()
    );
}

#[test]
fn jackson_original_scalar_types_and_output_budget_are_preserved() {
    use seata_at::{
        FastjsonSqlUndoLog, FastjsonUndoField, FastjsonUndoFieldValue as Value,
        FastjsonUndoRecords, SqlType, UndoCodecError, UndoCodecLimits,
    };
    use seata_datasource::DecimalValue;
    let fields = vec![
        FastjsonUndoField::new("small_long".into(), -5, false, Value::Long(1)),
        FastjsonUndoField::new(
            "decimal".into(),
            3,
            false,
            Value::Decimal(DecimalValue::new("0.0000001200").unwrap()),
        ),
        FastjsonUndoField::new("binary".into(), -3, false, Value::Binary(vec![0, 255, 127])),
        FastjsonUndoField::new("nullable".into(), 12, false, Value::Null),
    ];
    let logs = [FastjsonSqlUndoLog::new(
        SqlType::Insert,
        Some("t".into()),
        FastjsonUndoRecords::new(None, Vec::new()),
        FastjsonUndoRecords::new(Some("t".into()), vec![fields]),
    )];
    let parser = JacksonUndoLogParser::default();
    let wire = parser
        .encode_original(Some("origin"), 42, Some(&logs))
        .unwrap();
    let doc = parser.decode(&wire).unwrap().unwrap();
    let decoded = parser.decode_original_logs(&doc).unwrap().unwrap();
    assert!(logs[0].after_image() == decoded[0].after_image());
    let cap = UndoCodecLimits::new(wire.len() - 1, 4096, 32, 512, 4096).unwrap();
    assert!(matches!(
        JacksonUndoLogParser::new(cap).encode_original(Some("origin"), 42, Some(&logs)),
        Err(UndoCodecError::Budget {
            limit: "encoded output"
        })
    ));
}
#[test]
fn jackson_original_uses_java_utf8_escapes() {
    let parser = JacksonUndoLogParser::default();
    let xid = "\u{b}\u{1f}🐈\u{85}\u{2028}";
    let wire = parser.encode_original(Some(xid), 42, None).unwrap();
    let text = String::from_utf8(wire.clone()).unwrap();
    assert!(text.contains("\\u000B\\u001F\\uD83D\\uDC08\u{85}\u{2028}"));
    assert_eq!(parser.decode(&wire).unwrap().unwrap().xid(), Some(xid));
}

#[test]
fn jackson_original_projection_rechecks_input_budget() {
    use seata_at::{UndoCodecError, UndoCodecLimits};
    let parser = JacksonUndoLogParser::default();
    let doc = parser.decode(b"{}").unwrap().unwrap();
    let tight = JacksonUndoLogParser::new(UndoCodecLimits::new(1, 4096, 32, 512, 4096).unwrap());
    assert!(matches!(
        tight.decode_original_logs(&doc),
        Err(UndoCodecError::Budget {
            limit: "encoded bytes"
        })
    ));
}

fn jackson_array_field(
    name: &str,
    elements: Vec<Option<i64>>,
) -> (seata_at::SerialArray, seata_at::SerialArray, bool, String) {
    use seata_at::{FastjsonUndoField, FastjsonUndoFieldValue, SerialArray};
    let original = SerialArray::bigint("int8".into(), elements);
    let field = FastjsonUndoField::new(
        name.into(),
        2003,
        false,
        FastjsonUndoFieldValue::SerialArray(original.clone()),
    );
    let parser = JacksonUndoLogParser::default();
    let wire = parser.encode_original_field(&field).unwrap();
    let decoded = parser.decode_original_field(&wire).unwrap();
    let equivalent = field == decoded;
    (
        original,
        decoded.into_serial_array().unwrap(),
        equivalent,
        String::from_utf8(wire).unwrap(),
    )
}

fn java_array_text(elements: &[Option<i64>]) -> String {
    let values: Vec<_> = elements
        .iter()
        .map(|element| element.map_or_else(|| "null".into(), |value| value.to_string()))
        .collect();
    format!("[{}]", values.join(", "))
}

#[test]
fn jackson_source_test_serialize_and_deserialize_serial_array() {
    let (original, decoded, same, wire) =
        jackson_array_field("dept_ids", (1..=5).map(Some).collect());
    assert!(same);
    assert_eq!(original.base_type(), decoded.base_type());
    assert_eq!(original.base_type_name(), decoded.base_type_name());
    assert_eq!(original.elements(), decoded.elements());
    assert!(wire.contains(r#""elements":[1,2,3,4,5]"#));
    println!(
        "SEATA_JACKSON_ARRAY_VALUES\t{same}\t{}\t{}\t{}\t{wire}",
        decoded.base_type(),
        decoded.base_type_name(),
        java_array_text(decoded.elements())
    );
}

#[test]
fn jackson_source_test_serialize_and_deserialize_serial_array_with_nulls() {
    let (_, decoded, same, wire) =
        jackson_array_field("nullable_array", vec![Some(1), None, Some(3)]);
    assert!(same);
    let elements = decoded.elements();
    assert_eq!(elements.len(), 3);
    assert_eq!(elements[0], Some(1));
    assert!(elements[1].is_none());
    assert_eq!(elements[2], Some(3));
    assert!(wire.contains(r#""elements":[1,null,3]"#));
    println!(
        "SEATA_JACKSON_ARRAY_NULLS\t{same}\t{}\t{}\t{wire}",
        elements.len(),
        java_array_text(elements)
    );
}

#[test]
fn jackson_field_source_test_encode_jdbc_field_values() {
    use seata_at::{
        FastjsonUndoFieldValue as Common, JacksonUndoField, JacksonUndoFieldValue as V,
    };
    use seata_datasource::DecimalValue;

    let cases = [
        ("bigint_type", -5, V::Common(Common::Long(i64::MAX))),
        (
            "decimal_type",
            3,
            V::Common(Common::Decimal(
                DecimalValue::new("55555555555555555555.55555555555555555555").unwrap(),
            )),
        ),
        ("double_type", 8, V::Double(999999.999999999)),
        (
            "timestamp_type",
            93,
            V::Timestamp {
                epoch_seconds: 1_621_358_602,
                nanos: 111_000_000,
            },
        ),
        (
            "timestamp_type",
            93,
            V::Timestamp {
                epoch_seconds: 1_565_434_166,
                nanos: 926_554_000,
            },
        ),
        ("blob_type", 2004, V::SerialBlob(b"hello".to_vec())),
        ("clob_type", 2005, V::SerialClob("hello".into())),
        (
            "localdatetime_type",
            91,
            V::LocalDateTimeUtc {
                epoch_seconds: 1_621_358_602,
                nanos: 111_000_000,
            },
        ),
        (
            "localdatetime_type2",
            91,
            V::LocalDateTimeUtc {
                epoch_seconds: 1_621_358_602,
                nanos: 222_333_000,
            },
        ),
    ];
    let parser = JacksonUndoLogParser::default();
    for (index, (name, jdbc, value)) in cases.into_iter().enumerate() {
        let field = JacksonUndoField::new(name.into(), jdbc, false, value);
        let bytes = parser.encode_typed_field(&field).unwrap();
        let restored = parser.decode_typed_field(&bytes).unwrap();
        assert!(field == restored, "original field {name} at {index}");
        println!(
            "SEATA_JACKSON_FIELD\t{}\t{}\t{}\t{}",
            index + 1,
            restored.value().java_class(),
            field == restored,
            String::from_utf8(bytes).unwrap(),
        );
    }
}

#[test]
fn jackson_local_date_time_preserves_millis_when_submillis_is_separate() {
    use seata_at::JacksonUndoFieldValue as V;
    let wire = |class| {
        format!(
            r#"{{"@class":"org.apache.seata.rm.datasource.sql.struct.Field","name":"time","keyType":"NULL","type":93,"value":["{class}",[-1123,777222333]]}}"#
        )
    };
    let parser = JacksonUndoLogParser::default();
    let local = parser
        .decode_typed_field(wire("java.time.LocalDateTime").as_bytes())
        .unwrap();
    let timestamp = parser
        .decode_typed_field(wire("java.sql.Timestamp").as_bytes())
        .unwrap();
    assert!(
        local.value()
            == &V::LocalDateTimeUtc {
                epoch_seconds: -2,
                nanos: 877_222_333
            }
    );
    assert!(
        timestamp.value()
            == &V::Timestamp {
                epoch_seconds: -2,
                nanos: 777_222_333
            }
    );
}

#[test]
fn jackson_rejects_empty_serial_blob_encoder_and_preserves_signed_double_zero() {
    use seata_at::{JacksonUndoField, JacksonUndoFieldValue as V, UndoCodecError};
    let parser = JacksonUndoLogParser::default();
    let empty = JacksonUndoField::new("empty_blob".into(), 2004, false, V::SerialBlob(Vec::new()));
    assert!(matches!(
        parser.encode_typed_field(&empty),
        Err(UndoCodecError::Unsupported { .. })
    ));
    let explicit_empty = br#"{"@class":"org.apache.seata.rm.datasource.sql.struct.Field","name":"empty_blob","keyType":"NULL","type":2004,"value":["javax.sql.rowset.serial.SerialBlob",""]}"#;
    assert!(parser.decode_typed_field(explicit_empty).unwrap() == empty);
    let minus = JacksonUndoField::new("z".into(), 8, false, V::Double(-0.0));
    let plus = JacksonUndoField::new("z".into(), 8, false, V::Double(0.0));
    assert!(minus != plus);
    let wire = parser.encode_typed_field(&minus).unwrap();
    assert!(parser.decode_typed_field(&wire).unwrap() == minus);
}

fn dmdb_field(name: &str, timestamp: seata_at::DmdbTimestamp) -> seata_at::JacksonUndoField {
    seata_at::JacksonUndoField::new(
        name.into(),
        93,
        false,
        seata_at::JacksonUndoFieldValue::DmdbTimestamp(timestamp),
    )
}

fn dmdb_roundtrip(field: &seata_at::JacksonUndoField) -> (seata_at::JacksonUndoField, String) {
    let parser = JacksonUndoLogParser::default();
    let bytes = parser.encode_typed_field(field).unwrap();
    let actual = parser.decode_typed_field(&bytes).unwrap();
    (actual, String::from_utf8(bytes).unwrap())
}

#[test]
fn jackson_dmdb_source_test_serialize_and_deserialize_timestamp() {
    use seata_at::DmdbTimestamp as Dm;
    let cases = [
        (
            "dmdb_timestamp",
            Dm::from_zoned_utc(1_721_985_847, 0).unwrap(),
            true,
        ),
        (
            "dmdb_timestamp_type2",
            Dm::from_date_utc(1_721_985_847_000).unwrap(),
            false,
        ),
        (
            "dmdb_timestamp_nanos",
            Dm::from_zoned_utc(1_721_985_847, 12_345).unwrap(),
            true,
        ),
    ];
    for (index, (name, timestamp, expected)) in cases.into_iter().enumerate() {
        let original = dmdb_field(name, timestamp);
        let (decoded, wire) = dmdb_roundtrip(&original);
        let same = original == decoded;
        assert_eq!(same, expected);
        println!(
            "SEATA_JACKSON_DMDB\tordinary\t{}\t{same}\t{}\t{wire}",
            index + 1,
            decoded.value().java_class()
        );
    }
}

#[test]
fn jackson_dmdb_source_test_serialize_and_deserialize_timestamp_with_no_zone() {
    use seata_at::{DmdbTimestamp as Dm, JacksonUndoFieldValue as V};
    let original = Dm::from_date_utc(1_721_985_847_000).unwrap();
    let different = Dm::from_date_utc(1_721_985_847_001).unwrap();
    let field = dmdb_field("dmdb_timestamp_type", original);
    let (decoded, wire) = dmdb_roundtrip(&field);
    let V::DmdbTimestamp(decoded_time) = decoded.value() else {
        panic!("expected DmdbTimestamp");
    };

    let equal = original.same_instant(decoded_time);
    let different_equal = original.same_instant(&different);
    assert!(equal);
    assert!(!different_equal);
    println!(
        "SEATA_JACKSON_DMDB\tjackson\t1\t{equal}\t{}\t{wire}",
        decoded.value().java_class()
    );
    println!(
        "SEATA_JACKSON_DMDB\tjackson\t2\t{different_equal}\t{}\t{wire}",
        decoded.value().java_class()
    );
}

#[test]
fn jackson_dmdb_utc_rejects_unmapped_historical_driver_calendar() {
    use seata_at::{DmdbTimestamp, UndoCodecError};
    // 1582-10-15T00:00Z 是固定 JDK 的公历切换起点。
    let cutover = -12_219_292_800;
    assert!(DmdbTimestamp::from_zoned_utc(cutover, 0).is_ok());
    assert!(matches!(
        DmdbTimestamp::from_date_utc(cutover * 1000 - 1),
        Err(UndoCodecError::Unsupported { .. })
    ));
    let wire = format!(
        r#"{{"@class":"org.apache.seata.rm.datasource.sql.struct.Field","name":"time","keyType":"NULL","type":93,"value":["dm.jdbc.driver.DmdbTimestamp",{}]}}"#,
        cutover * 1000 - 1
    );
    assert!(matches!(
        JacksonUndoLogParser::default().decode_typed_field(wire.as_bytes()),
        Err(UndoCodecError::Unsupported { .. })
    ));
    assert!(matches!(
        DmdbTimestamp::from_zoned_utc(cutover - 1, 999_999_999),
        Err(UndoCodecError::Unsupported { .. })
    ));
}

#[test]
fn jackson_timestamp_source_test_branch_encode_and_decode() {
    use seata_at::{
        JacksonSqlUndoLog, JacksonUndoField, JacksonUndoFieldValue, JacksonUndoRecords,
    };
    let original = JacksonUndoFieldValue::Timestamp {
        epoch_seconds: 2_147_483,
        nanos: 999_999,
    };
    let field = JacksonUndoField::new("gmt_create".into(), 93, true, original.clone());
    let logs = [JacksonSqlUndoLog::new(
        None,
        None,
        Some(JacksonUndoRecords::empty(None)),
        Some(JacksonUndoRecords::new(
            Some("test".into()),
            vec![vec![field]],
        )),
    )];
    let parser = JacksonUndoLogParser::default();
    let wire = parser
        .encode_typed_branch(Some("192.168.0.1:8091:123456"), 123457, Some(&logs))
        .unwrap();
    let document = parser.decode(&wire).unwrap().unwrap();
    let decoded = parser.decode_typed_logs(&document).unwrap().unwrap();
    let actual = decoded[0].after_image().unwrap().rows()[0][0].value();
    assert!(actual == &original);
    assert!(decoded == logs);
    assert!(matches!(
        parser.restore_prepared(
            &document,
            "192.168.0.1:8091:123456",
            123457,
            &[],
            seata_at::MySqlJdbcProfile::Connector8Default,
        ),
        Err(seata_at::UndoCodecError::Shape {
            reason: "restored table name"
        })
    ));
    let wire = String::from_utf8(wire).unwrap();
    let forged = wire.replace(
        r#"TableRecords","tableName":"test""#,
        r#"TableRecords$EmptyTableRecords","tableName":"test""#,
    );
    assert_ne!(forged, wire);
    let forged = parser.decode(forged.as_bytes()).unwrap().unwrap();
    assert!(matches!(
        parser.decode_typed_logs(&forged),
        Err(seata_at::UndoCodecError::Unsupported { .. })
    ));
    let JacksonUndoFieldValue::Timestamp {
        epoch_seconds,
        nanos,
    } = actual
    else {
        panic!("expected Timestamp");
    };
    println!(
        "SEATA_JACKSON_BRANCH_TIMESTAMP\t{}\t{nanos}\t{}",
        epoch_seconds * 1000 + i64::from(nanos / 1_000_000),
        wire,
    );
}
