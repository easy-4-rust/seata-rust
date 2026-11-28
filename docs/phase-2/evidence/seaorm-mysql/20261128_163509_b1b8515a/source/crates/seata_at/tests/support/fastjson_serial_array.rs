//! 固定 Fastjson SerialArray 三个原用例的输入和断言。
#![allow(clippy::unwrap_used)]
use seata_at::{
    FastjsonSqlUndoLog, FastjsonUndoField, FastjsonUndoFieldValue, FastjsonUndoLogParser,
    FastjsonUndoRecords, SerialArray, SqlType,
};

fn array(values: Vec<Option<i64>>) -> SerialArray {
    SerialArray::bigint("int8".into(), values)
}

fn display(array: &SerialArray) -> String {
    let elements: Vec<_> = array
        .elements()
        .iter()
        .map(|value| value.map_or_else(|| "null".into(), |value| value.to_string()))
        .collect();
    format!("[{}]", elements.join(", "))
}

#[test]
fn fastjson_source_test_direct_serial_array_serialization() {
    let original = array((1..=5).map(Some).collect());
    let parser = FastjsonUndoLogParser::default();
    let bytes = parser.encode_serial_array(&original).unwrap();
    let decoded = parser.decode_serial_array(&bytes).unwrap();
    assert_eq!(original.base_type(), decoded.base_type());
    assert_eq!(original.base_type_name(), decoded.base_type_name());
    assert_eq!(original.elements(), decoded.elements());
    println!(
        "SEATA_FASTJSON_ARRAY_DIRECT\t{}\t{}\t{}\t{}",
        decoded.base_type(),
        decoded.base_type_name(),
        display(&decoded),
        std::str::from_utf8(&bytes).unwrap()
    );
}

fn embedded(name: &str, original: SerialArray) -> (String, bool, SerialArray) {
    let field = FastjsonUndoField::new(
        name.into(),
        2003,
        false,
        FastjsonUndoFieldValue::SerialArray(original.clone()),
    );
    let log = FastjsonSqlUndoLog::new(
        SqlType::Insert,
        Some("test_table".into()),
        FastjsonUndoRecords::new(None, Vec::new()),
        FastjsonUndoRecords::new(Some("test_table".into()), vec![vec![field.clone()]]),
    );
    let original_array = field.clone().into_serial_array().unwrap();
    let parser = FastjsonUndoLogParser::default();
    let bytes = parser
        .encode_branch(Some("test_xid"), 123456, Some(&[log]))
        .unwrap();
    let branch = parser.decode_branch(&bytes).unwrap().unwrap();
    let other = branch.serial_array_field(0, true, 0, 0).unwrap().unwrap();
    let same = field == other;
    let decoded = other.into_serial_array().unwrap();
    assert!(parser
        .decode_for_restore(&bytes, "test_xid", 123456)
        .is_err());
    // 用之前可恢复的物理镜像证明拒绝来自数组值，而非原源用例缺少物理主键。
    let wire_array = parser.encode_serial_array(&original_array).unwrap();
    let physical = std::str::from_utf8(super::FIXTURE).unwrap().replacen(
        "x'00FF7F'",
        std::str::from_utf8(&wire_array).unwrap(),
        1,
    );
    assert!(matches!(
        parser.decode_for_restore(physical.as_bytes(), super::XID, 42),
        Err(seata_at::UndoCodecError::Unsupported {
            kind: "Fastjson field object"
        })
    ));
    (String::from_utf8(bytes).unwrap(), same, decoded)
}

#[test]
fn fastjson_source_test_serialize_and_deserialize_serial_array() {
    let original = array((1..=5).map(Some).collect());
    let (wire, same, decoded) = embedded("dept_ids", original.clone());
    assert!(same);
    assert_eq!(original.base_type(), decoded.base_type());
    assert_eq!(original.base_type_name(), decoded.base_type_name());
    assert_eq!(original.elements(), decoded.elements());
    println!(
        "SEATA_FASTJSON_ARRAY_EMBEDDED\t{same}\t{}\t{}\t{}\t{wire}",
        decoded.base_type(),
        decoded.base_type_name(),
        display(&decoded)
    );
}

#[test]
fn fastjson_source_test_serialize_and_deserialize_serial_array_with_nulls() {
    let (wire, same, decoded) = embedded("nullable_array", array(vec![Some(1), None, Some(3)]));
    assert!(same);
    let elements = decoded.elements();
    assert_eq!(elements.len(), 3);
    assert_eq!(elements[0], Some(1));
    assert!(elements[1].is_none());
    assert_eq!(elements[2], Some(3));
    println!(
        "SEATA_FASTJSON_ARRAY_NULLS\t{same}\t{}\t{}\t{wire}",
        elements.len(),
        display(&decoded)
    );
}
