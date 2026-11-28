// 测试中的unwrap让任何非预期错误直接暴露为失败；生产API不使用unwrap。
#![allow(clippy::unwrap_used)]

use super::wire_reader::WireReader;
use super::wire_writer::WireWriter;
use crate::ProtocolError;

#[test]
fn signed_scalars_match_independent_big_endian_bytes() {
    let expected = [
        0xfe, 0x12, 0x34, 0xff, 0xfe, 0xff, 0xff, 0xff, 0xff, 0x80, 0, 0, 0, 0, 0, 0, 0,
    ];
    let mut writer = WireWriter::new(100);
    writer.u8(0xfe).unwrap();
    writer.u16(0x1234).unwrap();
    writer.i16(-2).unwrap();
    writer.i32(-1).unwrap();
    writer.i64(i64::MIN).unwrap();
    assert_eq!(writer.into_bytes(), expected);
    let mut reader = WireReader::new(&expected, 100).unwrap();
    assert_eq!(reader.u8().unwrap(), 0xfe);
    assert_eq!(reader.peek_u16().unwrap(), 0x1234);
    assert_eq!(reader.peek_u16().unwrap(), 0x1234);
    assert_eq!(reader.u16().unwrap(), 0x1234);
    assert_eq!(reader.i16().unwrap(), -2);
    assert_eq!(reader.i32().unwrap(), -1);
    assert_eq!(reader.i64().unwrap(), i64::MIN);
    reader.finish().unwrap();
}

#[test]
fn string_lengths_count_utf8_bytes_and_all_truncated_prefixes_fail() {
    let text = "事务🦀";
    let mut expected = vec![0, 10];
    expected.extend_from_slice(text.as_bytes());
    let mut writer = WireWriter::new(100);
    writer.string16(Some(text), "name").unwrap();
    assert_eq!(writer.into_bytes(), expected);
    for end in 0..expected.len() {
        let mut reader = WireReader::new(&expected[..end], 100).unwrap();
        assert!(matches!(
            reader.string16("name"),
            Err(ProtocolError::Truncated { .. })
        ));
    }
    let mut reader = WireReader::new(&expected, 100).unwrap();
    assert_eq!(reader.string16("name").unwrap().as_deref(), Some(text));
    reader.finish().unwrap();
}

#[test]
fn ordinary_zero_and_nullable_header_values_have_distinct_contracts() {
    let data = [0, 0, 0xff, 0xff, 0, 0];
    let mut reader = WireReader::new(&data, 100).unwrap();
    assert_eq!(reader.string16("xid").unwrap(), None);
    assert_eq!(reader.nullable_string16("header").unwrap(), None);
    assert_eq!(
        reader.nullable_string16("header").unwrap(),
        Some(String::new())
    );
    reader.finish().unwrap();
    let mut writer = WireWriter::new(100);
    writer.string16(None, "xid").unwrap();
    writer.nullable_string16(None, "header").unwrap();
    writer.nullable_string16(Some(""), "header").unwrap();
    assert_eq!(writer.into_bytes(), data);
}

#[test]
fn malformed_lengths_utf8_and_boolean_are_rejected_before_allocation() {
    for data in [&[0xff, 0xff][..], &[0x80, 0][..]] {
        assert!(matches!(
            WireReader::new(data, 100).unwrap().string16("x"),
            Err(ProtocolError::InvalidLength { .. })
        ));
    }
    assert!(matches!(
        WireReader::new(&[0xff, 0xff, 0xff, 0xff], 100)
            .unwrap()
            .string32("x"),
        Err(ProtocolError::InvalidLength { .. })
    ));
    assert!(matches!(
        WireReader::new(&[0x7f, 0xff, 0xff, 0xff], 100)
            .unwrap()
            .string32("x"),
        Err(ProtocolError::Truncated { .. })
    ));
    assert!(matches!(
        WireReader::new(&[0, 1, 0xff], 100).unwrap().string16("x"),
        Err(ProtocolError::InvalidUtf8 { .. })
    ));
    for value in [2u16, 255, 256, 65535] {
        assert!(matches!(
            WireReader::new(&value.to_be_bytes(), 100)
                .unwrap()
                .boolean16("lockable"),
            Err(ProtocolError::InvalidBoolean { .. })
        ));
    }
}

#[test]
fn budgets_prevent_partial_field_writes_and_arithmetic_overflow() {
    assert!(matches!(
        WireReader::new(&[1, 2], 1),
        Err(ProtocolError::LimitExceeded { .. })
    ));
    let mut reader = WireReader::new(&[1], 100).unwrap();
    assert!(matches!(
        reader.take(usize::MAX, "x"),
        Err(ProtocolError::Truncated { .. })
    ));
    assert_eq!(reader.remaining(), 1);
    let mut writer = WireWriter::new(4);
    writer.u8(7).unwrap();
    assert!(matches!(
        writer.string16(Some("é"), "x"),
        Err(ProtocolError::LimitExceeded { .. })
    ));
    assert_eq!(writer.into_bytes(), [7]);
}

#[test]
fn signed_short_string_limit_does_not_truncate_unicode() {
    let maximum = "a".repeat(32767);
    let mut writer = WireWriter::new(40000);
    writer.string16(Some(&maximum), "x").unwrap();
    let bytes = writer.into_bytes();
    assert_eq!(&bytes[..2], &[0x7f, 0xff]);
    assert_eq!(bytes.len(), 32769);
    let excessive = "é".repeat(16384);
    let mut writer = WireWriter::new(70000);
    assert!(matches!(
        writer.string16(Some(&excessive), "x"),
        Err(ProtocolError::InvalidLength { .. })
    ));
    assert!(writer.into_bytes().is_empty());
}

#[test]
fn string32_and_full_consumption_preserve_explicit_fields() {
    let expected = [0, 0, 0, 3, b'a', b'b', b'c', 0, 1];
    let mut writer = WireWriter::new(100);
    writer.string32(Some("abc"), "resource").unwrap();
    writer.boolean16(true).unwrap();
    assert_eq!(writer.into_bytes(), expected);
    let mut reader = WireReader::new(&expected, 100).unwrap();
    assert_eq!(reader.string32("resource").unwrap().as_deref(), Some("abc"));
    assert!(matches!(
        reader.finish(),
        Err(ProtocolError::TrailingBytes { remaining: 2 })
    ));
    assert!(reader.boolean16("lockable").unwrap());
    reader.finish().unwrap();
}
