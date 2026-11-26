//! 外部日志的值域与预算拒绝边界，不能以编译或自往返代替原Java比较。
#![allow(clippy::unwrap_used)]
use base64::{engine::general_purpose::STANDARD, Engine};
use seata_at::{JacksonUndoLogParser, UndoCodecError};
use serde_json::{json, Value};

fn document(value: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog",
        "sqlUndoLogs":["java.util.ArrayList",[{
            "@class":"org.apache.seata.rm.datasource.undo.SQLUndoLog",
            "afterImage":{
                "@class":"org.apache.seata.rm.datasource.sql.struct.TableRecords",
                "rows":["java.util.ArrayList",[{
                    "@class":"org.apache.seata.rm.datasource.sql.struct.Row",
                    "fields":["java.util.ArrayList",[{
                        "@class":"org.apache.seata.rm.datasource.sql.struct.Field",
                        "value":value
                    }]]
                }]]
            }
        }]]
    }))
    .unwrap()
}

#[test]
fn text_and_decoded_binary_allow_exactly_one_mib_and_reject_the_next_byte() {
    let parser = JacksonUndoLogParser::default();
    for length in [1_048_576, 1_048_577] {
        let text = document(json!("x".repeat(length)));
        let binary = document(json!(["[B", STANDARD.encode(vec![0xff; length])]));
        for bytes in [text, binary] {
            if length == 1_048_576 {
                assert!(parser.decode(&bytes).unwrap().is_some());
            } else {
                assert!(matches!(
                    parser.decode(&bytes),
                    Err(UndoCodecError::Budget { .. })
                ));
            }
        }
    }
    assert!(parser.decode(&document(json!(["[B", "%%%@"]))).is_err());
}

#[test]
fn declared_integer_wrappers_and_timestamp_nanos_reject_overflow_and_wrong_shapes() {
    let parser = JacksonUndoLogParser::default();
    for value in [
        json!(["java.lang.Byte", 128]),
        json!(["java.lang.Short", -32769]),
        json!(["java.lang.Integer", 2147483648_i64]),
        json!(["java.lang.Long", 9223372036854775808_u64]),
        json!(["java.math.BigInteger", 1.25]),
        json!(["java.math.BigDecimal", "1.25"]),
        json!(["java.sql.Timestamp", [0, 1000000000]]),
        json!(["java.time.LocalDateTime", [0, -1]]),
        json!(["java.sql.Timestamp", [0, 1, 2]]),
        json!(["java.lang.ProcessBuilder", "owned-fixture"]),
    ] {
        assert!(parser.decode(&document(value)).is_err());
    }
    for value in [
        json!(["java.lang.Byte", -128]),
        json!(["java.lang.Short", 32767]),
        json!(["java.lang.Integer", -2147483648_i64]),
        json!(["java.lang.Long", -9223372036854775808_i64]),
        json!(["java.sql.Timestamp", [0, 999999999]]),
        json!(["java.time.LocalDateTime", [0, 0]]),
    ] {
        assert!(parser.decode(&document(value)).unwrap().is_some());
    }
}

#[test]
fn jackson_number_digit_limit_counts_mantissa_and_exponent_but_not_signs_or_dot() {
    let parser = JacksonUndoLogParser::default();
    for (text, accepted) in [
        ("1".repeat(1000), true),
        ("1".repeat(1001), false),
        (format!("-{}", "1".repeat(1000)), true),
        (format!("-{}", "1".repeat(1001)), false),
        (format!("0.{}", "1".repeat(1000)), false),
        (format!("1.{}", "1".repeat(999)), true),
        (format!("1.{}", "1".repeat(1000)), false),
        (format!("-1.{}", "1".repeat(999)), true),
        (format!("1.{}E+0", "1".repeat(998)), true),
        (format!("1.{}E+0", "1".repeat(999)), false),
    ] {
        let number: serde_json::Number = text.parse().unwrap();
        let class = if text.contains(['.', 'E']) {
            "java.math.BigDecimal"
        } else {
            "java.math.BigInteger"
        };
        for value in [Value::Number(number.clone()), json!([class, number])] {
            let result = parser.decode(&document(value));
            if accepted {
                assert!(result.unwrap().is_some());
            } else {
                assert!(matches!(result, Err(UndoCodecError::Budget { .. })), "original Java rejected digit count");
            }
        }
    }
}
