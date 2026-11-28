//! 固定 Java Protostuff Undo 原用例。
#![allow(clippy::unwrap_used)]

#[path = "support/undo_h2_input.rs"]
mod h2;
use seata_at::{
    JacksonSqlUndoLog, JacksonUndoField, JacksonUndoFieldValue, JacksonUndoRecords,
    ProtostuffUndoLogParser,
};
use std::fmt::Write;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").unwrap();
        text
    })
}

#[test]
fn protostuff_source_test_decode_default_content() {
    let parser = ProtostuffUndoLogParser::default();
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
fn protostuff_source_test_timestamp_encode_and_decode() {
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
    let parser = ProtostuffUndoLogParser::default();
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
        "SEATA_PROTOSTUFF_TIMESTAMP\t{}\t{nanos}\t{}",
        epoch_seconds * 1000 + i64::from(nanos / 1_000_000),
        hex(&wire)
    );
}

#[test]
#[ignore = "requires fixed H2 JDBC input"]
fn protostuff_source_test_encode_and_decode() {
    let (xid, id, logs) = h2::typed_logs("testEncodeAndDecode");
    let parser = ProtostuffUndoLogParser::default();
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
fn protostuff_source_test_performance() {
    let (xid, id, logs) = h2::typed_logs("testPerformance");
    let parser = ProtostuffUndoLogParser::default();
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
fn fixed_java_protostuff_field_scalars_preserve_boxing_and_signed_values() {
    use seata_at::FastjsonUndoFieldValue as C;
    use seata_datasource::DecimalValue;
    use JacksonUndoFieldValue as V;
    let cases = [
        ("null", V::Common(C::Null), ""),
        (
            "Integer",
            V::Common(C::Integer(-17)),
            "2328efffffffffffffffff0124",
        ),
        (
            "Long",
            V::Common(C::Long(-2049)),
            "2330ffefffffffffffffff0124",
        ),
        ("Boolean", V::Common(C::Boolean(true)), "23080124"),
        ("Boolean", V::Common(C::Boolean(false)), "23080024"),
        ("byte[]", V::Common(C::Binary(vec![1, 2])), "235a02010224"),
        ("Double", V::Double(-0.5), "2341000000000000e0bf24"),
        (
            "BigDecimal",
            V::Common(C::Decimal(DecimalValue::new("7.250").unwrap())),
            "236205372e32353024",
        ),
        (
            "BigInteger",
            V::Common(C::Unsigned(u64::MAX)),
            "236a0900ffffffffffffffff24",
        ),
    ];
    let parser = ProtostuffUndoLogParser::default();
    for (class, value, encoded) in cases {
        let field = JacksonUndoField::new("n".into(), -3, false, value);
        let logs = [JacksonSqlUndoLog::new(
            None,
            None,
            None,
            Some(JacksonUndoRecords::new(None, vec![vec![field]])),
        )];
        let wire = parser.encode_branch(None, -1, Some(&logs)).unwrap();
        let actual = hex(&wire);
        assert_eq!(
            actual,
            format!(
                "10ffffffffffffffffff011b23130b0a016e100018fdffffffffffffffff01{encoded}0c14241c"
            )
        );
        let decoded = parser.decode_branch(&wire).unwrap();
        assert_eq!(decoded.branch_id(), -1);
        assert!(decoded.logs().unwrap() == logs);
        println!("SEATA_PROTOSTUFF_SCALAR\t{class}\t{actual}");
    }
}

#[test]
fn protostuff_rejects_overflow_bad_groups_and_duplicate_singular_fields() {
    use seata_at::{UndoCodecError, UndoCodecLimits};
    let parser = ProtostuffUndoLogParser::default();
    assert_eq!(parser.decode_branch(&[]).unwrap().branch_id(), 0);
    assert!(matches!(
        parser.decode_branch(&[0x1b, 0x24]),
        Err(UndoCodecError::Syntax)
    ));
    assert!(matches!(
        parser.decode_branch(&[0x1b]),
        Err(UndoCodecError::Syntax)
    ));
    let mut overflow = vec![0x10];
    overflow.extend([0xff; 9]);
    overflow.push(2);
    assert!(matches!(
        parser.decode_branch(&overflow),
        Err(UndoCodecError::Syntax)
    ));
    assert!(matches!(
        parser.decode_branch(&[0x10, 0, 0x10, 1]),
        Err(UndoCodecError::Shape {
            reason: "duplicate Protostuff singular field"
        })
    ));
    let caps = UndoCodecLimits::new(16, 8, 1, 1, 2).unwrap();
    let strict = ProtostuffUndoLogParser::new(caps);
    assert!(matches!(
        strict.decode_branch(&[0x1b, 0x1c]),
        Err(UndoCodecError::Budget {
            limit: "Protostuff nodes/depth"
        })
    ));
    assert!(matches!(
        strict.decode_branch(&[0x0a, 2, b'a', b'b']),
        Err(UndoCodecError::Budget {
            limit: "Protostuff scalar bytes"
        })
    ));
    assert!(matches!(
        strict.decode_branch(&[0x10, 0xff, 1]),
        Err(UndoCodecError::Budget {
            limit: "Protostuff scalar bytes"
        })
    ));
}

#[test]
fn protostuff_timestamp_delegate_requires_class_tag_before_payload() {
    let parser = ProtostuffUndoLogParser::default();
    let field = JacksonUndoField::new(
        "n".into(),
        93,
        false,
        JacksonUndoFieldValue::Timestamp {
            epoch_seconds: 1,
            nanos: 123,
        },
    );
    let logs = [JacksonSqlUndoLog::new(
        None,
        None,
        None,
        Some(JacksonUndoRecords::new(None, vec![vec![field]])),
    )];
    let mut wire = parser.encode_branch(None, 0, Some(&logs)).unwrap();
    let class = b"\xf2\x01\x12java.sql.Timestamp";
    let start = wire
        .windows(class.len())
        .position(|part| part == class)
        .unwrap();
    let end = start + class.len() + 14;
    wire[start..end].rotate_left(class.len());
    assert!(matches!(
        parser.decode_branch(&wire),
        Err(seata_at::UndoCodecError::Shape {
            reason: "Protostuff delegate discriminator order"
        })
    ));
}

#[test]
fn protostuff_double_keeps_raw_nan_bits() {
    let parser = ProtostuffUndoLogParser::default();
    let bits = 0x7ff8_0000_0000_0123;
    let field = JacksonUndoField::new(
        "n".into(),
        8,
        false,
        JacksonUndoFieldValue::Double(f64::from_bits(bits)),
    );
    let logs = [JacksonSqlUndoLog::new(
        None,
        None,
        None,
        Some(JacksonUndoRecords::new(None, vec![vec![field]])),
    )];
    let wire = parser.encode_branch(None, 0, Some(&logs)).unwrap();
    let doc = parser.decode_branch(&wire).unwrap();
    let value = doc.logs().unwrap()[0].after_image().unwrap().rows()[0][0].value();
    let JacksonUndoFieldValue::Double(value) = value else {
        panic!("expected Double")
    };
    assert_eq!(value.to_bits(), bits);
}
