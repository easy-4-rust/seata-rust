//! 固定 Fastjson 字段转换的五个参数化原用例及可空字段。
use super::assert_observation;
use seata_datasource::DecimalValue;
use seata_db_mysql::{DataCompareUtils, JavaCompareField, JavaCompareTemporal, JavaCompareValue};
use std::error::Error;

fn scalar_case(
    jdbc: i32,
    serialized: JavaCompareValue,
    actual: JavaCompareValue,
) -> Result<(), Box<dyn Error>> {
    let original = JavaCompareField::new(Some("value".into()), jdbc, serialized);
    let native = JavaCompareField::new(Some("VALUE".into()), jdbc, actual);
    let mut a = original.clone();
    let mut b = native.clone();
    let forward = DataCompareUtils::java_fastjson_fields_equal(
        Some(&mut a),
        Some(&mut b),
        JavaCompareTemporal::parse_jdbc_utc,
    )?;
    assert!(a.value() == native.value());
    let mut a = native.clone();
    let mut b = original;
    let reverse = DataCompareUtils::java_fastjson_fields_equal(
        Some(&mut a),
        Some(&mut b),
        JavaCompareTemporal::parse_jdbc_utc,
    )?;
    assert!(b.value() == native.value());
    assert_observation(&format!("DataCompareUtilsBehaviorTest#fastjsonValuesCompareWithJdbcTypesInEitherDirection:{jdbc}"),
                       &[forward,reverse],&[true,true]);
    Ok(())
}

fn temporal(
    jdbc: i32,
    kind: JavaCompareTemporal,
    serialized: &str,
    typed_input: &str,
) -> Result<(), Box<dyn Error>> {
    scalar_case(
        jdbc,
        JavaCompareValue::Text(serialized.into()),
        JavaCompareValue::Temporal(kind, kind.parse_jdbc_utc(&typed_input.into())?),
    )
}

#[test]
fn fastjson_date() -> Result<(), Box<dyn Error>> {
    temporal(
        91,
        JavaCompareTemporal::Date,
        "2024-03-02 00:00:00",
        "2024-03-02",
    )
}

#[test]
fn fastjson_time() -> Result<(), Box<dyn Error>> {
    temporal(92, JavaCompareTemporal::Time, "12:34:56", "12:34:56")
}

#[test]
fn fastjson_timestamp() -> Result<(), Box<dyn Error>> {
    temporal(
        93,
        JavaCompareTemporal::Timestamp,
        "2024-03-02 12:34:56.123",
        "2024-03-02 12:34:56.123",
    )
}

#[test]
fn fastjson_decimal() -> Result<(), Box<dyn Error>> {
    scalar_case(
        3,
        JavaCompareValue::Integer(42),
        JavaCompareValue::Decimal(DecimalValue::new("42")?),
    )
}

#[test]
fn fastjson_bigint() -> Result<(), Box<dyn Error>> {
    scalar_case(
        -5,
        JavaCompareValue::Integer(42),
        JavaCompareValue::Long(42),
    )
}

#[test]
fn fastjson_local_date_time_and_null_values() -> Result<(), Box<dyn Error>> {
    let input = "2024-03-02T12:34:56";
    let kind = JavaCompareTemporal::LocalDateTime;
    let mut text = JavaCompareField::new(
        Some("time".into()),
        93,
        JavaCompareValue::Text(input.into()),
    );
    let mut actual = JavaCompareField::new(
        Some("time".into()),
        93,
        JavaCompareValue::Temporal(kind, kind.parse_fastjson_utc(&input.into())?),
    );
    let mut int = JavaCompareField::new(Some("id".into()), 4, JavaCompareValue::Integer(1));
    let mut null = JavaCompareField::new(Some("id".into()), 4, JavaCompareValue::Null);
    let observed = [
        DataCompareUtils::java_fastjson_fields_equal(
            Some(&mut text),
            Some(&mut actual),
            JavaCompareTemporal::parse_fastjson_utc,
        )?,
        DataCompareUtils::java_fastjson_fields_equal(
            None,
            None,
            JavaCompareTemporal::parse_fastjson_utc,
        )?,
        DataCompareUtils::java_fastjson_fields_equal(
            Some(&mut int),
            None,
            JavaCompareTemporal::parse_fastjson_utc,
        )?,
        DataCompareUtils::java_fastjson_fields_equal(
            Some(&mut int),
            Some(&mut null),
            JavaCompareTemporal::parse_fastjson_utc,
        )?,
    ];
    assert!(text.value() == actual.value());
    assert_observation(
        "DataCompareUtilsBehaviorTest#fastjsonLocalDateTimeAndNullValues",
        &observed,
        &[true, true, false, false],
    );
    Ok(())
}
