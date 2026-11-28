//! 固定 Java Fory Undo 原用例。
#![allow(clippy::unwrap_used)]

#[path = "support/undo_h2_input.rs"]
mod h2;
use seata_at::ForyUndoLogParser;
use std::fmt::Write;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").unwrap();
        text
    })
}

#[test]
fn fory_source_test_decode_default_content() {
    let parser = ForyUndoLogParser::default();
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
#[ignore = "requires fixed H2 JDBC input"]
fn fory_source_test_encode_and_decode() {
    let (xid, id, logs) = h2::typed_logs("testEncodeAndDecode");
    let parser = ForyUndoLogParser::default();
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
fn fory_source_test_performance() {
    let (xid, id, logs) = h2::typed_logs("testPerformance");
    let parser = ForyUndoLogParser::default();
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
fn fixed_java_fory_strings_and_sli_longs_match() {
    let parser = ForyUndoLogParser::default();
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
            "SEATA_FORY_BOUNDARY\ttext\t{}\t{}",
            name.unwrap_or("null"),
            hex(&wire)
        );
    }
    for id in [i64::MIN, i64::MAX, -1, 0, 1] {
        let wire = parser.encode_branch(None, id, None).unwrap();
        assert_eq!(parser.decode_branch(&wire).unwrap().branch_id(), id);
        if id == i64::MIN || id == i64::MAX {
            assert_eq!(&wire[..4], &[13, 0, 0, 0]);
        }
        println!("SEATA_FORY_BOUNDARY\tid\t{id}\t{}", hex(&wire));
    }
}

#[test]
fn fory_rejects_invalid_metadata_offsets_descriptions_and_scalar_budgets() {
    use seata_at::{UndoCodecError, UndoCodecLimits};
    let parser = ForyUndoLogParser::default();
    let bytes = parser.default_content().unwrap();
    let caps = UndoCodecLimits::new(512, 32, 16, 3, 3).unwrap();
    assert!(matches!(
        ForyUndoLogParser::new(caps).decode_branch(&bytes),
        Err(UndoCodecError::Budget {
            limit: "Fory scalar bytes"
        })
    ));
    for end in 0..bytes.len() {
        assert!(parser.decode_branch(&bytes[..end]).is_err());
    }
    let mut invalid_offset = bytes.clone();
    invalid_offset[..4].copy_from_slice(&i32::MAX.to_le_bytes());
    assert!(matches!(
        parser.decode_branch(&invalid_offset),
        Err(UndoCodecError::Shape {
            reason: "Fory metadata offset"
        })
    ));
    let mut invalid_meta = bytes;
    let last = invalid_meta.last_mut().unwrap();
    *last ^= 1;
    assert!(matches!(
        parser.decode_branch(&invalid_meta),
        Err(UndoCodecError::Unsupported {
            kind: "Fory class definition"
        })
    ));
}

#[test]
fn fory_reuses_scalar_references_in_the_java_undo_value_field() {
    use seata_at::{
        FastjsonUndoFieldValue as C, JacksonSqlUndoLog, JacksonUndoField,
        JacksonUndoFieldValue as V, JacksonUndoRecords,
    };
    let parser = ForyUndoLogParser::default();
    let values = [
        ("Integer", V::Common(C::Integer(7))),
        ("Long", V::Common(C::Long(7))),
        ("Boolean", V::Common(C::Boolean(true))),
        ("String", V::Common(C::Text("same".into()))),
        ("Double", V::Double(-0.5)),
    ];
    for (name, value) in values {
        let row = vec![JacksonUndoField::new("v".into(), 4, false, value)];
        let logs = [JacksonSqlUndoLog::new(
            None,
            None,
            None,
            Some(JacksonUndoRecords::new(None, vec![row.clone(), row])),
        )];
        let mut wire = parser.encode_branch(None, 0, Some(&logs)).unwrap();
        let length = u32::from_le_bytes(wire[..4].try_into().unwrap()) as usize;
        if name == "Double" {
            // Java 探针明确把同一 Double 对象放入两行。Rust DTO 的独立浮点值
            // 没有共享对象身份，将第二个值替换为该探针的引用形式以验证读取。
            let double = [0, 0x2a, 0, 0, 0, 0, 0, 0, 0xe0, 0xbf];
            let second = wire[4..4 + length]
                .windows(double.len())
                .rposition(|bytes| bytes == double)
                .unwrap()
                + 4;
            wire.splice(second..second + double.len(), [0xfe, 8]);
            wire[..4].copy_from_slice(&((length - 8) as u32).to_le_bytes());
        }
        let length = u32::from_le_bytes(wire[..4].try_into().unwrap()) as usize;
        assert!(wire[4..4 + length].ends_with(&[0xfe, 8, 0xfd]), "{name}");
        assert!(parser.decode_branch(&wire).unwrap().logs().unwrap() == logs);
        println!("SEATA_FORY_SCALAR_REF\t{name}\t{}", hex(&wire));
    }
}
