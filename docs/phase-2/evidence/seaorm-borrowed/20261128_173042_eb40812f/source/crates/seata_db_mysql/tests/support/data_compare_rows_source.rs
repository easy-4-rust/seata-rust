//! 固定 Java 行级序列化器分派及原地变更的差分。
use super::text;
use seata_db_mysql::{DataCompareUtils, JavaCompareField, JavaCompareValue};
use seata_sqlparser::{MetadataError, MetadataLocale};

fn number(name: &str, value: JavaCompareValue) -> JavaCompareField {
    JavaCompareField::new(Some(name.into()), -5, value)
}

#[test]
fn compare_rows_selects_the_exact_serializer_name() -> Result<(), MetadataError> {
    for (label, serializer, expected) in [
        ("unset", None, false),
        ("different_case", Some("Fastjson"), false),
        ("fastjson", Some("fastjson"), true),
    ] {
        let mut left = [vec![number("id", JavaCompareValue::Integer(42))]];
        let mut right = [vec![number("id", JavaCompareValue::Long(42))]];
        let equal = DataCompareUtils::java_rows_equal_with_serializer(
            &["id".into()],
            Some(&mut left),
            Some(&mut right),
            &MetadataLocale::new("en"),
            serializer,
            |_, _| {
                Err(MetadataError::InvalidCompatibilityData(
                    "unexpected temporal input",
                ))
            },
        )?;
        assert_eq!(equal, expected);
        let converted = left[0][0].value() == &JavaCompareValue::Long(42);
        assert_eq!(converted, expected);
        println!("SEATA_COMPARE_ROWS\tserializer_{label}\t{equal}:{converted}");
    }
    Ok(())
}

#[test]
fn compare_rows_preserves_jdk_bucket_order_and_partial_conversion() -> Result<(), MetadataError> {
    let mut left = [
        vec![
            text("id", 12, "duplicate"),
            number("p", JavaCompareValue::Integer(21)),
        ],
        vec![
            text("a", 91, "   "),
            text("id", 12, "duplicate"),
            number("p", JavaCompareValue::Integer(42)),
        ],
    ];
    let mut right = [
        vec![text("id", 12, "duplicate")],
        vec![
            number("p", JavaCompareValue::Long(42)),
            text("id", 12, "duplicate"),
            text("a", 91, "2024-03-02"),
        ],
    ];
    let outcome = DataCompareUtils::java_rows_equal_with_serializer(
        &["id".into()],
        Some(&mut left),
        Some(&mut right),
        &MetadataLocale::new("en"),
        Some("fastjson"),
        |_, _| {
            Err(MetadataError::InvalidCompatibilityData(
                "unexpected temporal input",
            ))
        },
    );
    let error = outcome
        .err()
        .ok_or(MetadataError::InvalidCompatibilityData(
            "missing DATE failure",
        ))?;
    let class = error
        .java_exception_class()
        .ok_or(MetadataError::InvalidCompatibilityData(
            "missing DATE failure class",
        ))?;
    assert_eq!(class, "java.lang.ArrayIndexOutOfBoundsException");
    let previous_untouched = left[0][1].value() == &JavaCompareValue::Integer(21);
    let later_converted = left[1][2].value() == &JavaCompareValue::Long(42);
    assert!(previous_untouched && later_converted);
    assert!(left[1][0].value() == &JavaCompareValue::Text("   ".into()));
    println!("SEATA_COMPARE_ROWS\tduplicate_then_date_failure\t{class}:{previous_untouched}:{later_converted}");
    Ok(())
}

#[test]
fn compare_rows_matches_primary_keys_before_temporal_conversion() -> Result<(), MetadataError> {
    let mut left = [vec![text("id", 91, "2024-03-02 00:00:00")]];
    let mut right = [vec![text("id", 91, "2024-03-02")]];
    let equal = DataCompareUtils::java_rows_equal_with_serializer(
        &["id".into()],
        Some(&mut left),
        Some(&mut right),
        &MetadataLocale::new("en"),
        Some("fastjson"),
        |_, _| {
            Err(MetadataError::InvalidCompatibilityData(
                "unexpected temporal input",
            ))
        },
    )?;
    assert!(!equal);
    let unchanged = left[0][0].value() == &JavaCompareValue::Text("2024-03-02 00:00:00".into())
        && right[0][0].value() == &JavaCompareValue::Text("2024-03-02".into());
    assert!(unchanged);
    println!("SEATA_COMPARE_ROWS\tkeys_before_conversion\t{equal}:{unchanged}");
    Ok(())
}
