//! 固定 Java Kryo Undo 原用例。
#![allow(clippy::unwrap_used)]

#[path = "support/undo_h2_input.rs"]
mod h2;
use seata_at::{
    JacksonSqlUndoLog, JacksonUndoField, JacksonUndoFieldValue, JacksonUndoRecords,
    KryoUndoLogParser,
};
use std::fmt::Write;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").unwrap();
        text
    })
}

#[test]
fn kryo_source_test_decode_default_content() {
    let parser = KryoUndoLogParser::default();
    let bytes = parser.default_content().unwrap();
    let decoded = parser.decode_branch(&bytes).unwrap();
    assert!(decoded.xid().is_none());
    assert_eq!(decoded.branch_id(), 0);
    assert!(decoded.logs().is_none());
    println!(
        "SEATA_FASTJSON_DEFAULT\t{}\ttrue\ttrue\t0\ttrue",
        hex(&bytes)
    );
}

#[test]
fn kryo_source_test_timestamp_encode_and_decode() {
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
    let parser = KryoUndoLogParser::default();
    let wire = parser
        .encode_branch(Some("192.168.0.1:8091:123456"), 123457, Some(&logs))
        .unwrap();
    let document = parser.decode_branch(&wire).unwrap();
    let actual = document.logs().unwrap()[0].after_image().unwrap().rows()[0][0].value();
    assert!(actual == &original);
    let JacksonUndoFieldValue::Timestamp {
        epoch_seconds,
        nanos,
    } = actual
    else {
        panic!("expected Timestamp");
    };
    println!(
        "SEATA_KRYO_TIMESTAMP\t{}\t{nanos}\t{}",
        epoch_seconds * 1000 + i64::from(nanos / 1_000_000),
        hex(&wire)
    );
}

#[test]
#[ignore = "requires fixed H2 JDBC input"]
fn kryo_source_test_encode_and_decode() {
    let (xid, id, logs) = h2::typed_logs("testEncodeAndDecode");
    let parser = KryoUndoLogParser::default();
    let wire = parser.encode_branch(Some(&xid), id, Some(&logs)).unwrap();
    assert!(!wire.is_empty());
    let decoded = parser.decode_branch(&wire).unwrap();
    assert_eq!(decoded.branch_id(), id);
    assert_eq!(decoded.xid(), Some(xid.as_str()));
    let actual = decoded.logs().unwrap();
    assert_eq!(actual.len(), logs.len());
    let mut fields = vec![
        "true".into(),
        decoded.branch_id().to_string(),
        decoded.xid().unwrap().into(),
        actual.len().to_string(),
    ];
    for (expected, got) in logs.iter().zip(actual) {
        assert_eq!(expected.sql_type(), got.sql_type());
        assert_eq!(expected.table_name(), got.table_name());
        let before = expected.before_image() == got.before_image();
        let after = expected.after_image() == got.after_image();
        assert!(before && after);
        fields.extend([
            got.sql_type().unwrap().into(),
            got.table_name().unwrap().into(),
            before.to_string(),
            after.to_string(),
        ]);
    }
    fields.push(hex(&wire));
    println!("SEATA_FASTJSON_H2_ROUNDTRIP\t{}", fields.join("\t"));
}

#[test]
#[ignore = "requires fixed H2 JDBC input and 100000 original iterations"]
fn kryo_source_test_performance() {
    let (xid, id, logs) = h2::typed_logs("testPerformance");
    let parser = KryoUndoLogParser::default();
    let mut last = Vec::new();
    let mut observed = 0;
    let mut iterations = 0;
    for _ in 0..100_000 {
        last = parser.encode_branch(Some(&xid), id, Some(&logs)).unwrap();
        assert!(!last.is_empty());
        let decoded = parser.decode_branch(&last).unwrap();
        observed = decoded.branch_id();
        assert_eq!(observed, id);
        std::hint::black_box(decoded);
        iterations += 1;
    }
    println!(
        "SEATA_FASTJSON_H2_PERFORMANCE\t{iterations}\t{}\t{observed}\t{}",
        !last.is_empty(),
        hex(&last)
    );
}

#[test]
fn fixed_java_kryo_strings_and_nine_byte_longs_match() {
    let parser = KryoUndoLogParser::default();
    let names = [
        "".into(),
        "x".into(),
        "ID".into(),
        "é".into(),
        "中".into(),
        "🐈".into(),
        "x".repeat(32),
        "x".repeat(33),
        "x".repeat(70),
    ];
    for name in std::iter::once(None).chain(names.iter().map(|name| Some(name.as_str()))) {
        let wire = parser.encode_branch(name, 0, None).unwrap();
        assert_eq!(parser.decode_branch(&wire).unwrap().xid(), name);
        println!(
            "SEATA_KRYO_BOUNDARY\ttext\t{}\t{}",
            name.unwrap_or("null"),
            hex(&wire)
        );
    }
    for id in [i64::MIN, i64::MAX, -1, 0, 1] {
        let wire = parser.encode_branch(None, id, None).unwrap();
        assert_eq!(parser.decode_branch(&wire).unwrap().branch_id(), id);
        if id == i64::MIN || id == i64::MAX {
            assert_eq!(wire.len(), 13);
        }
        println!("SEATA_KRYO_BOUNDARY\tid\t{id}\t{}", hex(&wire));
    }
}

#[test]
fn kryo_rejects_cross_type_references_and_checks_budget() {
    use seata_at::{UndoCodecError, UndoCodecLimits};
    let parser = KryoUndoLogParser::default();
    let bytes = parser.default_content().unwrap();
    for stop in 0..bytes.len() {
        assert!(parser.decode_branch(&bytes[..stop]).is_err());
    }
    let mut invalid = bytes.clone();
    invalid[4] = 2;
    assert!(matches!(
        parser.decode_branch(&invalid),
        Err(UndoCodecError::Shape {
            reason: "Kryo string reference"
        })
    ));
    let mut trailing = bytes;
    trailing.push(1);
    assert!(matches!(
        parser.decode_branch(&trailing),
        Err(UndoCodecError::Syntax)
    ));
    let caps = UndoCodecLimits::new(64, 20, 8, 2, 10).unwrap();
    let wire = parser.encode_branch(Some("abc"), 0, None).unwrap();
    assert!(matches!(
        KryoUndoLogParser::new(caps).decode_branch(&wire),
        Err(UndoCodecError::Budget {
            limit: "Kryo scalar bytes"
        })
    ));
}

#[test]
fn kryo_counts_numeric_payload_in_single_and_total_scalar_budgets() {
    use seata_at::{UndoCodecError, UndoCodecLimits};
    let parser = KryoUndoLogParser::default();
    let wire = parser.encode_branch(None, i64::MIN, None).unwrap();
    let caps = UndoCodecLimits::new(64, 20, 8, 1, 1).unwrap();
    assert!(matches!(
        KryoUndoLogParser::new(caps).decode_branch(&wire),
        Err(UndoCodecError::Budget {
            limit: "Kryo scalar bytes"
        })
    ));
    let wire = parser.encode_branch(Some("hello"), i64::MIN, None).unwrap();
    let caps = UndoCodecLimits::new(64, 20, 8, 9, 13).unwrap();
    assert!(matches!(
        KryoUndoLogParser::new(caps).decode_branch(&wire),
        Err(UndoCodecError::Budget {
            limit: "Kryo total scalar bytes"
        })
    ));
}

#[test]
fn kryo_double_writes_the_java_canonical_nan() {
    let parser = KryoUndoLogParser::default();
    let field = JacksonUndoField::new(
        "value".into(),
        8,
        false,
        JacksonUndoFieldValue::Double(f64::from_bits(0x7ff8_0000_0000_0001)),
    );
    let logs = [JacksonSqlUndoLog::new(
        None,
        None,
        None,
        Some(JacksonUndoRecords::new(None, vec![vec![field]])),
    )];
    let wire = parser.encode_branch(None, 0, Some(&logs)).unwrap();
    let decoded = parser.decode_branch(&wire).unwrap();
    let value = decoded.logs().unwrap()[0].after_image().unwrap().rows()[0][0].value();
    let JacksonUndoFieldValue::Double(value) = value else {
        panic!("expected Double");
    };
    assert_eq!(value.to_bits(), 0x7ff8_0000_0000_0000);
}
