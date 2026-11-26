//! 固定 JDK JDBC 时间值的 UTC 差分。
use seata_db_mysql::{
    DataCompareUtils, JavaCompareField, JavaCompareTemporal,
    JavaCompareTemporal::{Date, Time, Timestamp},
    JavaCompareValue,
};
use seata_sqlparser::{JavaString, MetadataError};

const CASES: &[(&str, JavaCompareTemporal, &str, &str)] = &[
    ("date", Date, "2024-02-31", "1709337600:000000000"),
    ("date", Date, "1582-10-10", "-12218860800:000000000"),
    ("date", Date, "1500-2-29", "-14825894400:000000000"),
    ("date", Date, "0000-01-01", "-62167392000:000000000"),
    (
        "date",
        Date,
        "2024-02-32",
        "java.lang.IllegalArgumentException",
    ),
    ("date", Date, "2024-x-01", "java.lang.NumberFormatException"),
    ("date", Date, "٢٠٢٤-٢-٢٩", "1709164800:000000000"),
    ("date", Date, "2024-+2-+9", "1707436800:000000000"),
    ("time", Time, "24:00:00", "86400:000000000"),
    ("time", Time, "00:00:00", "0:000000000"),
    ("time", Time, "-1:+1:60", "-3480:000000000"),
    ("time", Time, "2147483647:00:00", "7730941129200:000000000"),
    (
        "time",
        Time,
        "-2147483648:00:00",
        "-7730940009600:000000000",
    ),
    (
        "time",
        Time,
        "2147483648:00:00",
        "java.lang.NumberFormatException",
    ),
    ("time", Time, "00:0:0:1", "java.lang.NumberFormatException"),
    (
        "timestamp",
        Timestamp,
        " 1969-12-31 23:59:59.123456789 ",
        "-1:123456789",
    ),
    (
        "timestamp",
        Timestamp,
        "1582-10-04 24:00:00.01",
        "-12219292800:010000000",
    ),
    (
        "timestamp",
        Timestamp,
        "1582-10-14 24:00:00",
        "-12219292800:000000000",
    ),
    (
        "timestamp",
        Timestamp,
        "9999-12-31 24:00:00",
        "253402300800:000000000",
    ),
    (
        "timestamp",
        Timestamp,
        "2024-2-9 0:0:0.١٢",
        "1707436800:120000000",
    ),
    (
        "timestamp",
        Timestamp,
        "2024-02-30 24:00:00.",
        "java.lang.IllegalArgumentException",
    ),
    (
        "timestamp",
        Timestamp,
        "2024-02-30 24:00:00.1234567890",
        "java.lang.IllegalArgumentException",
    ),
    (
        "timestamp",
        Timestamp,
        "2024-2-9 0:0:0.1x",
        "java.lang.NumberFormatException",
    ),
    (
        "timestamp",
        Timestamp,
        "2024-2-9 :0:0",
        "java.lang.NumberFormatException",
    ),
];

#[test]
fn jdbc_utc_values_and_exception_classes_match_the_fixed_jdk() -> Result<(), MetadataError> {
    for &(kind, class, input, expected) in CASES {
        let value = JavaCompareTemporal::parse_jdbc_utc(class, &JavaString::from(input));
        let actual = match value {
            Ok(text) => text.to_string(),
            Err(error) => error.java_exception_class().unwrap_or("missing").to_owned(),
        };
        assert_eq!(actual, expected, "{kind} {input}");
        println!("SEATA_COMPARE_JDBC_UTC\t{kind}\t{input}\t{actual}");
    }
    let field = |input: &str| {
        JavaCompareField::new(
            Some("value".into()),
            92,
            JavaCompareValue::Text(input.into()),
        )
    };
    let mut one_day = field("24:00:00");
    let mut zero = field("00:00:00");
    assert!(!DataCompareUtils::java_fastjson_fields_equal(
        Some(&mut one_day),
        Some(&mut zero),
        JavaCompareTemporal::parse_jdbc_utc,
    )?);
    Ok(())
}
