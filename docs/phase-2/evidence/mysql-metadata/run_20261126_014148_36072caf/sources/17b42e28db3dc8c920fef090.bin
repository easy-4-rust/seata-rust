//! 固定 Java 记录比较的空值、元数据与 Fastjson 转换边界。
use super::text;
use seata_db_mysql::{DataCompareUtils, JavaCompareField, JavaCompareRecordsMut, JavaCompareValue};
use seata_sqlparser::{MetadataError, MetadataLocale};

#[test]
fn records_compare_null_empty_and_missing_metadata_in_order() -> Result<(), MetadataError> {
    let locale = MetadataLocale::new("en");
    let compare = |left, right| {
        DataCompareUtils::java_records_equal_with_serializer(
            left,
            right,
            &locale,
            Some("fastjson"),
            |_, _| {
                Err(MetadataError::InvalidCompatibilityData(
                    "unexpected temporal input",
                ))
            },
        )
    };
    let both_absent = compare(None, None)?;
    assert!(both_absent);

    let make = |name: &str| JavaCompareRecordsMut::new(Some(name.into()), None, None);
    let one_absent = compare(None, Some(make("table")))?;
    let null_rows = compare(Some(make("table")), Some(make("TABLE")))?;
    let mut empty = [];
    let empty_vs_null = compare(
        Some(JavaCompareRecordsMut::new(
            Some("table".into()),
            None,
            Some(&mut empty),
        )),
        Some(make("table")),
    )?;
    let mut empty_left = [];
    let mut empty_right = [];
    let both_empty = compare(
        Some(JavaCompareRecordsMut::new(
            Some("table".into()),
            None,
            Some(&mut empty_left),
        )),
        Some(JavaCompareRecordsMut::new(
            Some("TABLE".into()),
            None,
            Some(&mut empty_right),
        )),
    )?;
    assert!(both_empty);
    let missing_right_name = compare(
        Some(make("table")),
        Some(JavaCompareRecordsMut::new(None, None, None)),
    )?;
    let missing_left_name = compare(
        Some(JavaCompareRecordsMut::new(None, None, None)),
        Some(make("table")),
    )
    .err()
    .and_then(|error| error.java_exception_class().map(str::to_owned));
    assert!(!one_absent && null_rows && !empty_vs_null && !missing_right_name);
    assert_eq!(
        missing_left_name.as_deref(),
        Some("java.lang.NullPointerException")
    );
    println!("SEATA_COMPARE_RECORDS\tnull_records\t{both_absent}:{one_absent}:{null_rows}:{empty_vs_null}:{missing_right_name}:{both_empty}");
    assert!(compare(Some(make("x")), Some(make("X")))?);

    let mut left = [vec![text("id", 12, "a")]];
    let mut right = [vec![text("id", 12, "a")]];
    let err = compare(
        Some(JavaCompareRecordsMut::new(
            Some("table".into()),
            None,
            Some(&mut left),
        )),
        Some(JavaCompareRecordsMut::new(
            Some("table".into()),
            None,
            Some(&mut right),
        )),
    )
    .err()
    .ok_or(MetadataError::InvalidCompatibilityData(
        "missing null metadata exception",
    ))?;
    let class = err.java_exception_class().unwrap_or("missing");
    assert_eq!(class, "java.lang.NullPointerException");
    println!(
        "SEATA_COMPARE_RECORDS\tnull_metadata\t{}:{class}",
        missing_left_name.as_deref().unwrap_or("missing")
    );
    Ok(())
}

#[test]
fn records_use_left_keys_then_update_the_original_fields() -> Result<(), MetadataError> {
    let make = |key| JavaCompareField::new(Some("payload".into()), -5, key);
    let mut left = [vec![
        text("id", 12, "same"),
        make(JavaCompareValue::Integer(42)),
    ]];
    let mut right = [vec![
        text("id", 12, "same"),
        make(JavaCompareValue::Long(42)),
    ]];
    let left_keys = ["id".into()];
    let right_keys = ["missing_column".into()];
    let locale = MetadataLocale::new("en");
    let equal = DataCompareUtils::java_records_equal_with_serializer(
        Some(JavaCompareRecordsMut::new(
            Some("table".into()),
            Some(&left_keys),
            Some(&mut left),
        )),
        Some(JavaCompareRecordsMut::new(
            Some("TABLE".into()),
            Some(&right_keys),
            Some(&mut right),
        )),
        &locale,
        Some("fastjson"),
        |_, _| {
            Err(MetadataError::InvalidCompatibilityData(
                "unexpected temporal input",
            ))
        },
    )?;
    let changed = left[0][1].value() == &JavaCompareValue::Long(42);
    assert!(equal && changed);
    // 如果错用右表的元数据，两行会碰撞并只留下末尾相同的行。
    let common = vec![text("id", 12, "common"), make(JavaCompareValue::Long(42))];
    let [first] = left;
    let mut left = [first, common.clone()];
    let mut right = [
        vec![
            text("id", 12, "different"),
            make(JavaCompareValue::Long(42)),
        ],
        common,
    ];
    let unequal = DataCompareUtils::java_records_equal_with_serializer(
        Some(JavaCompareRecordsMut::new(
            Some("table".into()),
            Some(&left_keys),
            Some(&mut left),
        )),
        Some(JavaCompareRecordsMut::new(
            Some("table".into()),
            Some(&right_keys),
            Some(&mut right),
        )),
        &locale,
        Some("fastjson"),
        |_, _| {
            Err(MetadataError::InvalidCompatibilityData(
                "unexpected temporal input",
            ))
        },
    )?;
    assert!(!unequal);
    println!("SEATA_COMPARE_RECORDS\tleft_metadata_and_updates\t{equal}:{changed}:{unequal}");
    Ok(())
}
