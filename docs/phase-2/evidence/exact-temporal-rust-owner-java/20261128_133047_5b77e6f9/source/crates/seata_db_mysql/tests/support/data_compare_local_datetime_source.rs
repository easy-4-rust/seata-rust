//! 本地日期时间与固定 JDK 的严格 ISO 语法、日期范围及纳秒差分。
use seata_db_mysql::JavaCompareTemporal;
use seata_sqlparser::JavaString;

const INVALID: &str = "java.time.format.DateTimeParseException";

#[test]
fn local_datetime_values_and_errors_match_fixed_jdk() {
    let cases = [
        ("2024-03-02T12:34", "2024:03:02:12:34:00:000000000"),
        ("2024-03-02t12:34:56.", "2024:03:02:12:34:56:000000000"),
        ("2024-03-02T12:34:56.1", "2024:03:02:12:34:56:100000000"),
        (
            "2024-03-02T12:34:56.100000000",
            "2024:03:02:12:34:56:100000000",
        ),
        (
            "2024-03-02T12:34:56.000000001",
            "2024:03:02:12:34:56:000000001",
        ),
        ("0000-02-29T00:00", "0:02:29:00:00:00:000000000"),
        ("-0001-01-01T23:59:59", "-1:01:01:23:59:59:000000000"),
        ("+10000-02-29T00:00", "10000:02:29:00:00:00:000000000"),
        ("+02024-02-29T00:00", "2024:02:29:00:00:00:000000000"),
        (
            "+0999999999-12-31T23:59:59.999999999",
            "999999999:12:31:23:59:59:999999999",
        ),
        (
            "-999999999-01-01T00:00",
            "-999999999:01:01:00:00:00:000000000",
        ),
        ("+1000000000-01-01T00:00", INVALID),
        ("-0000-01-01T00:00", INVALID),
        ("+2024-01-01T00:00", INVALID),
        ("10000-01-01T00:00", INVALID),
        ("1582-10-10T00:00", "1582:10:10:00:00:00:000000000"),
        ("1500-02-29T00:00", INVALID),
        ("2024-04-31T00:00", INVALID),
        ("2024-13-01T00:00", INVALID),
        ("2024-01-01T24:00", INVALID),
        ("2024-01-01T00:60", INVALID),
        ("2024-01-01T00:00:60", INVALID),
        ("2024-01-01T00:00:00.1234567890", INVALID),
        ("2024-01-01T00:00.", INVALID),
        ("2024-01-01T00:00Z", INVALID),
        ("2024-01-01T00:00+08:00", INVALID),
        ("2024-01-01T00:00 ", INVALID),
        ("2024-1-01T00:00", INVALID),
        ("٢٠٢٤-01-01T00:00", INVALID),
    ];
    for (input, expected) in cases {
        let result =
            JavaCompareTemporal::LocalDateTime.parse_fastjson_utc(&JavaString::from(input));
        let actual = match result {
            Ok(value) => value.to_string(),
            Err(error) => error.java_exception_class().unwrap_or("missing").to_owned(),
        };
        assert_eq!(actual, expected, "{input}");
        println!("SEATA_COMPARE_LOCAL_TIME\t{input}\t{actual}");
    }
}
