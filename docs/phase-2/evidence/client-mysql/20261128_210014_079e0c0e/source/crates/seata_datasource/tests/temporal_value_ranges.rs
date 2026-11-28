//! 来自真实 MySQL DATE/TIME 边界与负微秒反例的值模型验收。
#![allow(clippy::unwrap_used)]
use seata_datasource::{InternalTime, LocalDate};

#[test]
fn full_gregorian_date_range_preserves_the_jdk_cutover_gap() {
    let mut days = 0;
    for year in 1000..=9999 {
        for month in 1..=12 {
            for day in 1..=31 {
                if let Ok(value) = LocalDate::new(year, month, day) {
                    assert_eq!(LocalDate::parse_sql(&value.to_sql_string()).unwrap(), value);
                    days += 1;
                }
            }
        }
    }
    assert_eq!(days, 3_287_182);
    for text in [
        "1000-01-01",
        "1582-10-05",
        "1582-10-10",
        "1582-10-14",
        "9999-12-31",
    ] {
        assert_eq!(LocalDate::parse_sql(text).unwrap().to_sql_string(), text);
    }
}

#[test]
fn date_rejects_zero_invalid_calendar_and_non_calendar_input() {
    for text in [
        "0000-00-00",
        "0999-12-31",
        "10000-01-01",
        "1500-02-29",
        "1900-02-29",
        "2000-00-01",
        "2000-01-00",
        "2000-01-01 ",
        "2000-01-01 00:00:00",
        "2000-1-01",
        "2000-01-😃",
    ] {
        assert!(LocalDate::parse_sql(text).is_err(), "{text}");
    }
    assert_eq!(
        LocalDate::new(2000, 2, 29).unwrap().to_sql_string(),
        "2000-02-29"
    );
}

#[test]
fn signed_time_preserves_negative_microseconds_and_durations_beyond_one_day() {
    for (text, micros, fsp, expected) in [
        ("-00:00:00.000001", -1, 6, "-00:00:00.000001"),
        ("00:00:00.000001", 1, 6, "00:00:00.000001"),
        ("-25:01:02", -90_062_000_000, 6, "-25:01:02.000000"),
        ("-838:59:59", -3_020_399_000_000, 6, "-838:59:59.000000"),
        ("838:59:59", 3_020_399_000_000, 6, "838:59:59.000000"),
        (
            "-838:59:58.999999",
            -3_020_398_999_999,
            6,
            "-838:59:58.999999",
        ),
        ("12:34:56.1234", 45_296_123_400, 4, "12:34:56.1234"),
        ("-00:00:00.000000", 0, 0, "00:00:00"),
    ] {
        let value = InternalTime::parse_sql(text).unwrap();
        assert_eq!(value.microseconds(), micros);
        assert_eq!(value.to_sql_string(fsp).unwrap(), expected);
        assert_eq!(value, InternalTime::from_microseconds(micros).unwrap());
    }
}

#[test]
fn time_range_rejects_fraction_beyond_838_59_59_and_malformed_values() {
    for value in [i64::MIN, -3_020_399_000_001, 3_020_399_000_001, i64::MAX] {
        assert!(InternalTime::from_microseconds(value).is_err());
    }
    for text in [
        "",
        "-",
        "838:59:59.000001",
        "-838:59:59.000001",
        "839:00:00",
        "-839:00:00",
        "12:60:00",
        "12:00:60",
        "12:00:00.",
        "12:00:00.0000001",
        "1:00:00",
        "12:0:00",
        "12:00:00 ",
        "12:00:00.微秒",
    ] {
        assert!(InternalTime::parse_sql(text).is_err(), "{text}");
    }
}

#[test]
fn time_fractional_precision_is_explicit_and_never_silently_truncated() {
    for fsp in 0..=6 {
        let micros = if fsp == 0 {
            0
        } else {
            10i64.pow(6 - u32::from(fsp))
        };
        let value = InternalTime::from_microseconds(-micros).unwrap();
        let text = value.to_sql_string(fsp).unwrap();
        assert_eq!(InternalTime::parse_sql(&text).unwrap(), value);
        if fsp < 6 {
            assert!(InternalTime::from_microseconds(-1)
                .unwrap()
                .to_sql_string(fsp)
                .is_err());
        }
    }
    assert!(InternalTime::from_microseconds(0)
        .unwrap()
        .to_sql_string(7)
        .is_err());
}

#[test]
fn deterministic_signed_range_samples_round_trip_exact_microseconds() {
    let max = InternalTime::MAX_MICROSECONDS;
    let mut state = 17u64;
    for _ in 0..20_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let micros = (state % (max as u64 * 2 + 1)) as i64 - max;
        let value = InternalTime::from_microseconds(micros).unwrap();
        assert_eq!(
            InternalTime::parse_sql(&value.to_sql_string(6).unwrap()).unwrap(),
            value
        );
    }
}
