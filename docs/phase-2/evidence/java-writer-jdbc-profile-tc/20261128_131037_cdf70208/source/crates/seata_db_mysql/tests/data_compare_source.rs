//! 固定原 Java DataCompareUtils 的紧邻比较、可空镜像和 Map 断言。
#[path = "support/data_compare_fastjson_source.rs"]
mod fastjson_source;

use seata_db_mysql::{DataCompareUtils, JavaCompareField, JavaCompareRecords, JavaCompareValue};
use seata_sqlparser::{JavaString, MetadataError, MetadataLocale};

fn text(name: &str, jdbc: i32, value: &str) -> JavaCompareField {
    JavaCompareField::new(
        Some(name.into()),
        jdbc,
        JavaCompareValue::Text(value.into()),
    )
}

fn assert_observation(case: &str, values: &[bool], expected: &[bool]) {
    assert_eq!(values, expected, "{case}");
    let actual: String = values
        .iter()
        .map(|item| if *item { '1' } else { '0' })
        .collect();
    println!("SEATA_DATA_COMPARE\t{case}\t{actual}");
}

#[test]
fn is_field_equals() {
    let zero = text("name", 0, "111");
    let null = JavaCompareField::new(Some("name".into()), 0, JavaCompareValue::Null);
    let other_null = JavaCompareField::new(Some("Name".into()), 0, JavaCompareValue::Null);
    let blob = || {
        JavaCompareField::new(
            Some("information".into()),
            2004,
            JavaCompareValue::Binary(b"hello world".to_vec()),
        )
    };
    let eq = DataCompareUtils::java_fields_equal;
    let actual = [
        eq(Some(&zero), None),
        eq(None, Some(&zero)),
        eq(Some(&zero), Some(&text("name", 1, "111"))),
        eq(Some(&zero), Some(&text("name", 0, "222"))),
        eq(Some(&zero), Some(&text("age", 0, "222"))),
        eq(Some(&zero), Some(&null)),
        eq(Some(&zero), Some(&text("Name", 0, "111"))),
        eq(Some(&null), Some(&other_null)),
        eq(Some(&blob()), Some(&blob())),
    ];
    assert_observation(
        "DataCompareUtilsTest#isFieldEquals",
        &actual,
        &[false, false, false, false, false, false, true, true, true],
    );
}

#[test]
fn is_records_equals() -> Result<(), MetadataError> {
    let locale = MetadataLocale::new("en");
    let keys = ["pk".into()];
    let before_rows = [vec![text("pk", 1, "12345"), text("age", 1, "18")]];
    let before = JavaCompareRecords::new("table_name".into(), &keys, Some(&before_rows));
    let compare = |name: &str, rows: Option<&[Vec<JavaCompareField>]>| {
        DataCompareUtils::java_records_equal(
            Some(&before),
            Some(&JavaCompareRecords::new(name.into(), &keys, rows)),
            &locale,
        )
    };
    let good = before_rows.clone();
    let wrong_key = [vec![text("pk", 1, "23456"), text("age", 1, "18")]];
    let wrong_name = [vec![text("pk", 1, "12345"), text("sex", 1, "18")]];
    let wrong_age = [vec![text("pk", 1, "12345"), text("age", 1, "19")]];
    let extra = [
        vec![text("pk", 1, "12345"), text("18", 1, "19")],
        vec![text("pk", 1, "12346")],
    ];
    let empty = JavaCompareRecords::new("table_name".into(), &keys, Some(&[]));
    let actual = [
        DataCompareUtils::java_records_equal(Some(&before), None, &locale)?,
        DataCompareUtils::java_records_equal(None, Some(&before), &locale)?,
        compare("table_name1", Some(&[]))?,
        compare("table_name", Some(&[]))?,
        compare("table_name", Some(&good))?,
        compare("table_name", Some(&wrong_key))?,
        compare("table_name", Some(&wrong_name))?,
        compare("table_name", Some(&wrong_age))?,
        compare("table_name", Some(&extra))?,
        DataCompareUtils::java_records_equal(Some(&empty), Some(&empty), &locale)?,
    ];
    assert_observation(
        "DataCompareUtilsTest#isRecordsEquals",
        &actual,
        &[
            false, false, false, false, true, false, false, false, false, true,
        ],
    );
    Ok(())
}

#[test]
fn is_rows_equals() -> Result<(), MetadataError> {
    let locale = MetadataLocale::new("en");
    let keys = ["pk".into()];
    let row = |key| vec![text("pk", 1, key)];
    let left = [row("12345")];
    let compare = |l, r| DataCompareUtils::java_rows_equal(&keys, l, r, &locale);
    let actual = [
        compare(Some(&left), None)?,
        compare(None, Some(&left))?,
        compare(Some(&left), Some(&[row("12345")]))?,
        compare(Some(&[row("23456")]), Some(&left))?,
        compare(Some(&left), Some(&[row("12345"), row("12346")]))?,
    ];
    assert_observation(
        "DataCompareUtilsTest#isRowsEquals",
        &actual,
        &[false, false, true, false, false],
    );
    Ok(())
}

fn assert_map(
    case: &str,
    keys: &[JavaString],
    rows: &[Vec<JavaCompareField>],
    key: &str,
) -> Result<(), MetadataError> {
    let map = DataCompareUtils::java_row_list_to_map(rows, keys, &MetadataLocale::new("en"))?;
    assert_eq!(map.len(), 3);
    let present = map.contains_key(&key.into());
    assert!(present);
    println!(
        "SEATA_DATA_COMPARE\t{case}\t{}:{}",
        map.len(),
        u8::from(present)
    );
    Ok(())
}

#[test]
fn test_row_list_to_map_with_single_pk() -> Result<(), MetadataError> {
    let rows = ["1", "2", "3"].map(|value| vec![text("id", 1, value)]);
    assert_map(
        "DataCompareUtilsTest#testRowListToMapWithSinglePk",
        &["id".into()],
        &rows,
        "1",
    )
}

#[test]
fn test_row_list_to_map_with_multip_pk() -> Result<(), MetadataError> {
    let rows = [("1", "2"), ("3", "4"), ("5", "6")]
        .map(|(a, b)| vec![text("id1", 1, a), text("id2", 1, b)]);
    assert_map(
        "DataCompareUtilsTest#testRowListToMapWithMultipPk",
        &["id1".into(), "id2".into()],
        &rows,
        "1_2",
    )
}

#[test]
fn rows_match_by_composite_key_and_report_missing_columns_or_rows() -> Result<(), MetadataError> {
    let int = |name: &str, value| {
        JavaCompareField::new(Some(name.into()), 4, JavaCompareValue::Integer(value))
    };
    let original = [vec![
        int("tenant", 2),
        int("id", 1),
        text("value", 12, "before"),
    ]];
    let missing = |id| [vec![int("id", id), int("tenant", 2)]];
    let locale = MetadataLocale::new("en");
    let keys = ["id".into(), "tenant".into()];
    let actual = [
        DataCompareUtils::java_rows_equal(&keys, Some(&original), Some(&missing(1)), &locale)?,
        DataCompareUtils::java_row_list_to_map(&original, &keys, &locale)?
            .contains_key(&"1_2".into()),
        DataCompareUtils::java_rows_equal(&keys, Some(&original), Some(&missing(3)), &locale)?,
    ];
    assert_observation(
        "DataCompareUtilsBehaviorTest#rowsMatchByCompositeKeyAndReportMissingColumnsOrRows",
        &actual,
        &[false, true, false],
    );
    Ok(())
}

#[test]
fn java_map_preserves_original_duplicate_and_collision_behavior() -> Result<(), MetadataError> {
    let locale = MetadataLocale::new("en");
    let keys = ["id".into(), "tenant".into()];
    let row = |id: &str, tenant: &str| vec![text("id", 12, id), text("tenant", 12, tenant)];
    let first = row("first", "tenant");
    let second = row("second", "tenant");
    let unique = [first.clone(), second];
    let duplicate = [first.clone(), first];
    let a = row("a_b", "c");
    let b = row("a", "b_c");
    let collision = [a.clone(), b.clone()];
    let reverse = [b.clone(), a.clone()];
    let equal = |left: &[Vec<JavaCompareField>], right: &[Vec<JavaCompareField>]| {
        DataCompareUtils::java_rows_equal(&keys, Some(left), Some(right), &locale)
    };
    for (name, actual, expected) in [
        ("right_duplicate", equal(&unique, &duplicate)?, false),
        ("left_duplicate", equal(&duplicate, &unique)?, true),
        ("both_duplicate", equal(&duplicate, &duplicate)?, true),
        (
            "colliding_key_reordered",
            equal(&collision, &reverse)?,
            false,
        ),
        ("different_composite_key", equal(&[a], &[b])?, false),
    ] {
        assert_eq!(actual, expected);
        println!("SEATA_COMPARE_EXTRA\t{name}\t{actual}");
    }
    let size = DataCompareUtils::java_row_list_to_map(&collision, &keys, &locale)?.len();
    assert_eq!(size, 1);
    println!("SEATA_COMPARE_EXTRA\tcolliding_mapper_rows\t{size}");
    Ok(())
}

#[test]
fn fastjson_date_only_spaces_throws_before_parsing() -> Result<(), MetadataError> {
    let mut left = text("date", 91, "   ");
    let mut right = text("date", 91, "2024-03-02");
    let mut calls = 0;
    let result =
        DataCompareUtils::java_fastjson_fields_equal(Some(&mut left), Some(&mut right), |_, _| {
            calls += 1;
            Ok("parsed".into())
        });
    let error = result.err().ok_or(MetadataError::InvalidCompatibilityData(
        "expected DATE split to fail",
    ))?;
    let class = error
        .java_exception_class()
        .ok_or(MetadataError::InvalidCompatibilityData(
            "missing DATE exception class",
        ))?;
    assert_eq!(class, "java.lang.ArrayIndexOutOfBoundsException");
    assert!(error
        .object_exception()
        .is_some_and(|exception| exception.is_instance_of("java.lang.IndexOutOfBoundsException")));
    let message = error
        .java_message()
        .ok_or(MetadataError::InvalidCompatibilityData(
            "missing DATE exception message",
        ))?;
    assert_eq!(
        message,
        &JavaString::from("Index 0 out of bounds for length 0")
    );
    assert_eq!(calls, 0);
    assert!(left.value() == &JavaCompareValue::Text("   ".into()));
    assert!(right.value() == &JavaCompareValue::Text("2024-03-02".into()));
    println!("SEATA_COMPARE_EXTRA\tspace_only_date\t{class}:{message}");
    Ok(())
}

#[test]
fn decimal_row_keys_follow_big_decimal_to_string() -> Result<(), MetadataError> {
    use seata_datasource::DecimalValue;

    for (input, expected) in [
        ("0.0000010", "0.0000010"),
        ("0.0000001", "1E-7"),
        ("-0.0000001200", "-1.200E-7"),
        ("0.0000000", "0E-7"),
        ("12.0000000", "12.0000000"),
    ] {
        let rows = [vec![JavaCompareField::new(
            Some("id".into()),
            3,
            JavaCompareValue::Decimal(DecimalValue::new(input).map_err(|_| {
                MetadataError::InvalidCompatibilityData("invalid decimal test input")
            })?),
        )]];
        let mapped = DataCompareUtils::java_row_list_to_map(
            &rows,
            &["id".into()],
            &MetadataLocale::new("en"),
        )?;
        assert_eq!(mapped.len(), 1);
        let key = mapped
            .keys()
            .next()
            .ok_or(MetadataError::InvalidCompatibilityData(
                "missing decimal row key",
            ))?;
        assert_eq!(key, &JavaString::from(expected), "{input}");
        println!("SEATA_COMPARE_EXTRA\tdecimal_key_{input}\t{key}");
    }
    Ok(())
}

#[path = "support/data_compare_rows_source.rs"]
mod rows_source;

#[path = "support/data_compare_records_source.rs"]
mod records_source;

#[path = "support/data_compare_jdbc_utc_source.rs"]
mod jdbc_utc_source;

#[path = "support/data_compare_local_datetime_source.rs"]
mod local_datetime_source;
