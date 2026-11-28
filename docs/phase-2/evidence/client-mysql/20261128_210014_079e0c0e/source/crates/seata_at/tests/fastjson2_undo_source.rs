//! 固定 Fastjson2 JSONB 原用例。
#![allow(clippy::unwrap_used)]

#[path = "support/undo_h2_input.rs"]
mod h2;
use seata_at::{
    Fastjson2UndoLogParser, FastjsonUndoFieldValue as Common, JacksonSqlUndoLog, JacksonUndoField,
    JacksonUndoFieldValue as V, JacksonUndoRecords, SerialArray,
};
use std::fmt::Write;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").unwrap();
        text
    })
}

#[test]
fn fastjson2_source_test_decode_default_content() {
    let parser = Fastjson2UndoLogParser::default();
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

fn array_field(name: &str, values: Vec<Option<i64>>) -> (Vec<u8>, SerialArray) {
    let arr = SerialArray::bigint("int8".into(), values);
    let field = JacksonUndoField::new(
        name.into(),
        2003,
        false,
        V::Common(Common::SerialArray(arr)),
    );
    let logs = [JacksonSqlUndoLog::new(
        Some("UPDATE".into()),
        Some("test_table".into()),
        Some(JacksonUndoRecords::new(
            Some("test_table".into()),
            vec![vec![field]],
        )),
        Some(JacksonUndoRecords::empty(None)),
    )];
    let parser = Fastjson2UndoLogParser::default();
    let wire = parser
        .encode_branch(Some("192.168.0.1:8091:123456"), 123457, Some(&logs))
        .unwrap();
    let decoded = parser.decode_branch(&wire).unwrap();
    assert!(decoded.logs().unwrap() == logs);
    let V::Common(Common::SerialArray(value)) =
        decoded.logs().unwrap()[0].before_image().unwrap().rows()[0][0].value()
    else {
        panic!("expected BIGINT SerialArray");
    };
    (wire, value.clone())
}

fn array_text(values: &[Option<i64>]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| v.map_or_else(|| "null".into(), |n| n.to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[test]
fn fastjson2_source_test_array_values() {
    let (wire, array) = array_field("dept_ids", (1..=5).map(Some).collect());
    assert_eq!(array.base_type(), -5);
    assert_eq!(array.base_type_name(), "int8");
    assert_eq!(
        array.elements(),
        &[Some(1), Some(2), Some(3), Some(4), Some(5)]
    );
    println!(
        "SEATA_FASTJSON2_ARRAY_VALUES\ttrue\t{}\t{}\t{}\t{}",
        array.base_type(),
        array.base_type_name(),
        array_text(array.elements()),
        hex(&wire)
    );
}

#[test]
fn fastjson2_source_test_array_nulls() {
    let (wire, array) = array_field("nullable_array", vec![Some(1), None, Some(3)]);
    assert_eq!(array.elements().len(), 3);
    assert_eq!(array.elements()[0], Some(1));
    assert!(array.elements()[1].is_none());
    assert_eq!(array.elements()[2], Some(3));
    println!(
        "SEATA_FASTJSON2_ARRAY_NULLS\ttrue\t{}\t{}\t{}",
        array.elements().len(),
        array_text(array.elements()),
        hex(&wire)
    );
}

#[test]
#[ignore = "requires fixed H2 JDBC input"]
fn fastjson2_source_test_encode_and_decode() {
    let (xid, id, logs) = h2::typed_logs("testEncodeAndDecode");
    let parser = Fastjson2UndoLogParser::default();
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
fn fastjson2_source_test_performance() {
    let (xid, id, logs) = h2::typed_logs("testPerformance");
    let parser = Fastjson2UndoLogParser::default();
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
fn jsonb_truncations_invalid_symbols_and_budget_are_rejected() {
    use seata_at::{UndoCodecError, UndoCodecLimits};
    let parser = Fastjson2UndoLogParser::default();
    let bytes = parser.default_content().unwrap();
    for stop in 0..bytes.len() {
        assert!(parser.decode_branch(&bytes[..stop]).is_err());
    }
    let cap = UndoCodecLimits::new(bytes.len() - 1, 500, 32, 512, 4096).unwrap();
    assert!(matches!(
        Fastjson2UndoLogParser::new(cap).decode_branch(&bytes),
        Err(UndoCodecError::Budget {
            limit: "encoded bytes"
        })
    ));
    let cap = UndoCodecLimits::new(4096, 500, 32, 48, 4096).unwrap();
    assert!(matches!(
        Fastjson2UndoLogParser::new(cap).decode_branch(&bytes),
        Err(UndoCodecError::Budget {
            limit: "JSONB scalar bytes"
        })
    ));
    let mut invalid = bytes.clone();
    let offset = invalid.len() - 3;
    invalid[offset] = 7;
    assert!(matches!(
        parser.decode_branch(&invalid),
        Err(UndoCodecError::Shape {
            reason: "JSONB symbol declaration order"
        })
    ));
    let mut trailing = bytes;
    trailing.push(0xaf);
    assert!(matches!(
        parser.decode_branch(&trailing),
        Err(UndoCodecError::Syntax)
    ));
}

#[test]
fn jsonb_java_compact_strings_choose_latin1_utf8_and_utf16le() {
    let parser = Fastjson2UndoLogParser::default();
    for (value, encoded) in [
        ("é", "4ae9"),
        ("中", "7c022d4e"),
        ("🐈", "7c043dd808dc"),
        ("a中b", "7a0561e4b8ad62"),
    ] {
        let wire = parser.encode_branch(Some(value), 0, None).unwrap();
        assert!(hex(&wire).ends_with(&format!("{encoded}a5")), "{value}");
        assert_eq!(parser.decode_branch(&wire).unwrap().xid(), Some(value));
    }
}
