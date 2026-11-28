use time::format_description::well_known::Rfc2822;
use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcDateTime, UtcOffset};
const DATE: &str = "Fri, 21 Nov 1997 09:55:06 -0600";
fn nested(depth: usize) -> String {
    "(".repeat(depth) + "a" + &")".repeat(depth) + " " + DATE
}
fn rejected(name: &str, text: &str) -> bool {
    match name {
        "date" => Date::parse(text, &Rfc2822).is_err(),
        "time" => Time::parse(text, &Rfc2822).is_err(),
        "offset" => UtcOffset::parse(text, &Rfc2822).is_err(),
        "primitive" => PrimitiveDateTime::parse(text, &Rfc2822).is_err(),
        "offset_datetime" => OffsetDateTime::parse(text, &Rfc2822).is_err(),
        "utc_datetime" => UtcDateTime::parse(text, &Rfc2822).is_err(),
        _ => panic!("unknown test entry point"),
    }
}
const ENTRIES: [&str; 6] = [
    "date",
    "time",
    "offset",
    "primitive",
    "offset_datetime",
    "utc_datetime",
];
#[test]
fn normal_rfc2822_values_preserve_all_entry_points() {
    for entry in ENTRIES {
        assert!(!rejected(entry, DATE), "{entry}");
    }
    let expected = time::macros::datetime!(1997-11-21 09:55:06 -06:00);
    assert_eq!(OffsetDateTime::parse(DATE, &Rfc2822).unwrap(), expected);
    assert_eq!(
        UtcDateTime::parse(DATE, &Rfc2822).unwrap().unix_timestamp(),
        expected.unix_timestamp()
    );
    assert_eq!(Date::parse(DATE, &Rfc2822).unwrap(), expected.date());
    assert_eq!(Time::parse(DATE, &Rfc2822).unwrap(), expected.time());
    assert_eq!(UtcOffset::parse(DATE, &Rfc2822).unwrap(), expected.offset());
}
#[test]
fn nesting_limit_matches_upstream_boundary() {
    for depth in [1, 2, 16, 30, 31, 32, 33, 64, 256] {
        let text = nested(depth);
        for entry in ENTRIES {
            assert_eq!(rejected(entry, &text), depth >= 32, "{entry} depth={depth}");
        }
    }
}
#[test]
fn shallow_comments_and_folding_preserve_valid_inputs() {
    for text in [
        "(a)".repeat(10_000) + DATE,
        "(a\r\n \tb) ".to_owned() + DATE,
    ] {
        for entry in ENTRIES {
            assert!(!rejected(entry, &text), "{entry}");
        }
    }
}
#[test]
fn quoted_parentheses_are_not_nested_comments() {
    let text = "(".to_owned() + &"\\(\\)".repeat(10_000) + ") " + DATE;
    for entry in ENTRIES {
        assert!(!rejected(entry, &text), "{entry}");
    }
}
#[test]
fn malformed_nested_and_quoted_comments_return_errors() {
    for prefix in [
        "(".repeat(31),
        "(unterminated\\".into(),
        ")(".into(),
        "(a\r\nb) ".into(),
    ] {
        for entry in ENTRIES {
            assert!(rejected(entry, &(prefix.clone() + DATE)), "{entry}");
        }
    }
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SerdeDates {
    #[serde(with = "time::serde::rfc2822")]
    required: OffsetDateTime,
    #[serde(with = "time::serde::rfc2822::option")]
    optional: Option<OffsetDateTime>,
}
#[test]
fn serde_rfc2822_and_option_preserve_regular_values() {
    let input = serde_json::json!({"required":DATE,"optional":DATE});
    let value: SerdeDates = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), input);
    let empty: SerdeDates =
        serde_json::from_value(serde_json::json!({"required":DATE,"optional":null})).unwrap();
    assert!(empty.optional.is_none());
}
#[test]
fn serde_rfc2822_rejects_excess_nesting() {
    for depth in [32, 33, 10_000] {
        assert!(serde_json::from_value::<SerdeDates>(
            serde_json::json!({"required":nested(depth),"optional":null})
        )
        .is_err());
        assert!(serde_json::from_value::<SerdeDates>(
            serde_json::json!({"required":DATE,"optional":nested(depth)})
        )
        .is_err());
    }
}
#[test]
fn all_public_parsers_reject_large_nested_comments() {
    let text = nested(100_000);
    for entry in ENTRIES {
        assert!(rejected(entry, &text), "{entry}");
    }
}
#[test]
#[ignore = "runner isolates original stack exhaustion in a child process"]
fn attack_worker() {
    let entry = std::env::var("SEATA_TIME_ENTRY").unwrap();
    let text = nested(100_000);
    let worker = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            assert!(
                rejected(&entry, &text),
                "nested RFC 2822 unexpectedly accepted"
            );
            println!("bounded parse error: {entry} depth=100000 stack=262144");
        })
        .unwrap();
    worker.join().unwrap();
}
