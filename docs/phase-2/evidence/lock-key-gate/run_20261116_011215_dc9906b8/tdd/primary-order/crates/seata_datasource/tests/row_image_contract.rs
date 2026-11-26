//! 精确行值、完整schema、独立主键序与原子预算契约。
#![allow(clippy::unwrap_used)]
use seata_datasource::{
    ColumnMeta, ColumnType, DecimalValue, Field, FieldValue, ImageError, Row, TableMeta,
    TableRecords,
};

fn column(
    name: &str,
    kind: ColumnType,
    nullable: bool,
    ordinal: u32,
    pk: Option<u32>,
) -> ColumnMeta {
    ColumnMeta::new(
        name.into(),
        kind,
        nullable,
        ordinal,
        pk,
        if matches!(kind, ColumnType::Text | ColumnType::Binary) {
            Some(64)
        } else {
            None
        },
        (kind == ColumnType::Decimal).then_some(65),
        (kind == ColumnType::Decimal).then_some(30),
    )
    .unwrap()
}
fn meta() -> TableMeta {
    TableMeta::new(
        "Account".into(),
        vec![
            column("b", ColumnType::SignedInteger, false, 1, Some(2)),
            column("a", ColumnType::UnsignedInteger, false, 2, Some(1)),
            column("note", ColumnType::Text, true, 3, None),
        ],
    )
    .unwrap()
}
fn row(table: &TableMeta) -> Row {
    Row::new(
        table,
        vec![
            Field::new(&table.columns()[0], FieldValue::Signed(i64::MIN)).unwrap(),
            Field::new(&table.columns()[1], FieldValue::Unsigned(u64::MAX)).unwrap(),
            Field::new(&table.columns()[2], FieldValue::Null).unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn decimal_preserves_all_65_digits_and_scale_without_float() {
    let text = format!("{}.{}", "9".repeat(35), "1".repeat(30));
    let value = DecimalValue::new(&text).unwrap();
    assert_eq!(value.as_str(), text);
    assert_eq!(value.scale(), 30);
    assert_eq!(value.integer_digits(), 35);
    let col = column("amount", ColumnType::Decimal, false, 1, None);
    assert_eq!(
        Field::new(&col, FieldValue::Decimal(value.clone()))
            .unwrap()
            .value(),
        &FieldValue::Decimal(value)
    );
}
#[test]
fn decimal_rejects_noncanonical_or_out_of_budget_inputs() {
    for text in [
        "", "+1", " 1", "1 ", "1e2", ".1", "1.", "01", "00.1", "-0", "-0.000", "NaN", "--1",
        "1.2.3", "１",
    ] {
        assert!(DecimalValue::new(text).is_err(), "accepted {text}");
    }
    assert!(DecimalValue::new(&"9".repeat(66)).is_err());
    assert!(DecimalValue::new(&format!("0.{}", "1".repeat(31))).is_err());
    assert!(DecimalValue::new(&format!("{}.{}", "1".repeat(36), "2".repeat(30))).is_err());
}
#[test]
fn decimal_column_capacity_and_exact_scale_are_required() {
    let col = ColumnMeta::new(
        "d".into(),
        ColumnType::Decimal,
        false,
        1,
        None,
        None,
        Some(2),
        Some(2),
    )
    .unwrap();
    assert!(Field::new(
        &col,
        FieldValue::Decimal(DecimalValue::new("0.12").unwrap())
    )
    .is_ok());
    for text in ["1.12", "0.1", "0.123", "0"] {
        assert!(Field::new(&col, FieldValue::Decimal(DecimalValue::new(text).unwrap())).is_err());
    }
    assert!(DecimalValue::new("-0.10").is_ok());
}
#[test]
fn null_empty_text_and_empty_binary_are_distinct() {
    let text = column("text", ColumnType::Text, true, 1, None);
    let binary = column("binary", ColumnType::Binary, true, 1, None);
    assert_ne!(
        Field::new(&text, FieldValue::Null).unwrap().value(),
        Field::new(&text, FieldValue::Text(String::new()))
            .unwrap()
            .value()
    );
    assert_ne!(
        Field::new(&binary, FieldValue::Null).unwrap().value(),
        Field::new(&binary, FieldValue::Binary(Vec::new()))
            .unwrap()
            .value()
    );
    let raw = vec![0, 255, 0, 128];
    assert_eq!(
        Field::new(&binary, FieldValue::Binary(raw.clone()))
            .unwrap()
            .value(),
        &FieldValue::Binary(raw)
    );
}
#[test]
fn integer_extremes_remain_typed_and_exact() {
    let table = meta();
    let value = row(&table);
    assert_eq!(
        value.field("B").unwrap().value(),
        &FieldValue::Signed(i64::MIN)
    );
    assert_eq!(
        value.field("a").unwrap().value(),
        &FieldValue::Unsigned(u64::MAX)
    );
    assert_eq!(value.value_bytes(), 16);
}
#[test]
fn wrong_value_domains_and_nonnullable_null_are_rejected() {
    let signed = column("v", ColumnType::SignedInteger, false, 1, None);
    for value in [
        FieldValue::Null,
        FieldValue::Unsigned(1),
        FieldValue::Text("1".into()),
        FieldValue::Binary(vec![1]),
        FieldValue::Decimal(DecimalValue::new("1").unwrap()),
    ] {
        assert!(Field::new(&signed, value).is_err());
    }
    assert!(Field::new(
        &column("v", ColumnType::UnsignedInteger, false, 1, None),
        FieldValue::Signed(-1)
    )
    .is_err());
}
#[test]
fn utf8_and_binary_lengths_use_octets() {
    let text = ColumnMeta::new(
        "v".into(),
        ColumnType::Text,
        true,
        1,
        None,
        Some(3),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        Field::new(&text, FieldValue::Text("中".into()))
            .unwrap()
            .value_bytes(),
        3
    );
    assert!(Field::new(&text, FieldValue::Text("中文".into())).is_err());
    let binary = ColumnMeta::new(
        "v".into(),
        ColumnType::Binary,
        true,
        1,
        None,
        Some(3),
        None,
        None,
    )
    .unwrap();
    assert!(Field::new(&binary, FieldValue::Binary(vec![0; 4])).is_err());
}
#[test]
fn single_field_payload_has_independent_hard_budget() {
    let col = ColumnMeta::new(
        "v".into(),
        ColumnType::Binary,
        true,
        1,
        None,
        Some(2_000_000),
        None,
        None,
    )
    .unwrap();
    assert!(Field::new(&col, FieldValue::Binary(vec![7; 1_048_576])).is_ok());
    assert!(matches!(
        Field::new(&col, FieldValue::Binary(vec![7; 1_048_577])),
        Err(ImageError::Budget { .. })
    ));
}
#[test]
fn row_requires_complete_ordered_and_exact_column_metadata() {
    let table = meta();
    let good = row(&table);
    let fields = good.fields().to_vec();
    assert!(Row::new(&table, fields[..2].to_vec()).is_err());
    let mut reverse = fields.clone();
    reverse.swap(0, 1);
    assert!(Row::new(&table, reverse).is_err());
    let mut duplicate = fields.clone();
    duplicate[1] = fields[0].clone();
    assert!(Row::new(&table, duplicate).is_err());
    let mut incompatible = fields;
    let changed = column("note", ColumnType::Text, false, 3, None);
    incompatible[2] = Field::new(&changed, FieldValue::Text(String::new())).unwrap();
    assert!(Row::new(&table, incompatible).is_err());
}
#[test]
fn primary_keys_follow_index_order_and_nonkeys_follow_physical_order() {
    let table = meta();
    let value = row(&table);
    assert_eq!(
        value.primary_keys().map(Field::name).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(
        value
            .non_primary_keys()
            .map(Field::name)
            .collect::<Vec<_>>(),
        ["note"]
    );
    assert!(value.field("B").unwrap().is_key("b"));
    assert!(value.field("absent").is_none());
}
#[test]
fn records_reject_other_schema_and_keep_original_state() {
    let table = meta();
    let mut records = TableRecords::new(table.clone());
    records.add(row(&table)).unwrap();
    let other = TableMeta::new("account".into(), table.columns().to_vec()).unwrap();
    assert!(records.add(row(&other)).is_err());
    assert_eq!(records.rows().len(), 1);
    assert_eq!(records.value_bytes(), 16);
    assert_eq!(records.table_meta(), &table);
}
#[test]
fn row_and_byte_budgets_fail_atomically() {
    let table = meta();
    let mut rows = TableRecords::with_limits(table.clone(), 1, 64).unwrap();
    rows.add(row(&table)).unwrap();
    assert!(rows.add(row(&table)).is_err());
    assert_eq!(rows.rows().len(), 1);
    let mut bytes = TableRecords::with_limits(table.clone(), 4, 31).unwrap();
    bytes.add(row(&table)).unwrap();
    assert!(bytes.add(row(&table)).is_err());
    assert_eq!(bytes.value_bytes(), 16);
    assert_eq!(bytes.rows().len(), 1);
    let mut exact = TableRecords::with_limits(table.clone(), 1, 16).unwrap();
    exact.add(row(&table)).unwrap();
    for (count, size) in [
        (0, 1),
        (1, 0),
        (1025, 1),
        (1, 8_388_609),
        (usize::MAX, usize::MAX),
    ] {
        assert!(TableRecords::with_limits(table.clone(), count, size).is_err());
    }
}
#[test]
fn empty_records_keep_metadata_and_no_values() {
    let table = meta();
    let records = TableRecords::new(table.clone());
    assert!(records.rows().is_empty());
    assert_eq!(records.value_bytes(), 0);
    assert_eq!(records.table_name(), "Account");
    assert_eq!(records.table_meta(), &table);
}
#[test]
fn debug_and_errors_do_not_disclose_values() {
    let secret = "sensitive-cell-123";
    let table = meta();
    let mut fields = row(&table).fields().to_vec();
    fields[2] = Field::new(&table.columns()[2], FieldValue::Text(secret.into())).unwrap();
    let value = Row::new(&table, fields).unwrap();
    let mut records = TableRecords::new(table);
    records.add(value.clone()).unwrap();
    for debug in [
        format!("{value:?}"),
        format!("{records:?}"),
        format!("{:?}", FieldValue::Text(secret.into())),
        format!("{:?}", DecimalValue::new("123456.789").unwrap()),
    ] {
        assert!(!debug.contains(secret));
        assert!(!debug.contains("123456.789"));
        assert!(!debug.contains(&i64::MIN.to_string()));
    }
    let invalid = DecimalValue::new(secret).unwrap_err();
    assert!(!format!("{invalid:?} {invalid}").contains(secret));
}
