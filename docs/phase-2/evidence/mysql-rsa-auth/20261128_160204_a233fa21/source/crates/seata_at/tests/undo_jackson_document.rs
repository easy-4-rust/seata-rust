//! 原Java生成对象图必须保留精确值、空分支和空镜像；不能用事务模型限制抹掉原数据。
#![allow(clippy::unwrap_used)]
use seata_at::{JacksonUndoLogParser, UndoCodecError, UndoCodecLimits};

const NATIVE: &[&[u8]] = &[
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/default/undo_integer_types.json"),
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/default/undo_text_types.json"),
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/default/undo_binary_types.json"),
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/default/undo_decimal_types.json"),
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/default/undo_composite.json"),
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/default/undo_empty.json"),
    include_bytes!(
        "../../../tests/fixtures/undo/java_8_0_27/numeric_tinyint/undo_integer_types.json"
    ),
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/numeric_tinyint/undo_text_types.json"),
    include_bytes!(
        "../../../tests/fixtures/undo/java_8_0_27/numeric_tinyint/undo_binary_types.json"
    ),
    include_bytes!(
        "../../../tests/fixtures/undo/java_8_0_27/numeric_tinyint/undo_decimal_types.json"
    ),
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/numeric_tinyint/undo_composite.json"),
    include_bytes!("../../../tests/fixtures/undo/java_8_0_27/numeric_tinyint/undo_empty.json"),
];

#[test]
fn original_source_value_probes_keep_temporal_arrays_and_nullable_sql_identity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/undo/java_source_probes");
    let parser = JacksonUndoLogParser::default();
    let mut count = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let bytes = std::fs::read(&path).unwrap();
            let document = parser.decode(&bytes).unwrap().unwrap();
            let encoded = parser.encode(Some(&document)).unwrap();
            assert_eq!(parser.decode(&encoded).unwrap().unwrap(), document);
            if path.file_name().unwrap() == "probe-branch-source.json" {
                assert_eq!(document.branch_id(), 641789253);
                assert!(document.sql_undo_logs().unwrap()[0]["tableName"].is_null());
            }
            count += 1;
        }
    }
    assert_eq!(count, 17);
}
#[test]
fn original_native_golden_documents_keep_ids_types_and_exact_numbers() {
    let parser = JacksonUndoLogParser::default();
    for bytes in NATIVE {
        let document = parser.decode(bytes).unwrap().unwrap();
        assert_eq!(document.branch_id(), i64::MAX);
        assert_eq!(document.xid(), Some("127.0.0.1:8091:42"));
        assert_eq!(document.sql_undo_logs().unwrap().len(), 3);
        let encoded = parser.encode(Some(&document)).unwrap();
        assert_eq!(parser.decode(&encoded).unwrap().unwrap(), document);
        assert_eq!(
            parser.encode(Some(&document)).unwrap(),
            encoded,
            "deterministic object order"
        );
    }
}
#[test]
fn original_default_bytes_and_null_are_distinct() {
    let parser = JacksonUndoLogParser::default();
    assert_eq!(parser.default_content(), b"{}");
    let default = parser.decode(b"{}").unwrap().unwrap();
    assert_eq!(default.branch_id(), 0);
    assert_eq!(default.xid(), None);
    assert!(default.sql_undo_logs().is_none());
    assert!(parser.decode(b"null").unwrap().is_none());
    assert_eq!(parser.encode(None).unwrap(), b"null");
    let encoded = parser.encode(Some(&default)).unwrap();
    assert_ne!(encoded, b"{}");
    assert_eq!(parser.decode(&encoded).unwrap().unwrap(), default);
}

#[test]
fn empty_collection_container_depth_is_the_same_for_encode_and_decode() {
    let profile = seata_at::MySqlJdbcProfile::Connector8NumericTinyInt;
    let bytes = JacksonUndoLogParser::default()
        .encode_prepared("x", 1, &[], profile)
        .unwrap();
    let document = JacksonUndoLogParser::default()
        .decode(&bytes)
        .unwrap()
        .unwrap();
    for depth in [1, 2, 3] {
        let parser =
            JacksonUndoLogParser::new(UndoCodecLimits::new(4096, 100, depth, 128, 4096).unwrap());
        if depth < 3 {
            assert!(matches!(
                parser.decode(&bytes),
                Err(UndoCodecError::Budget { .. })
            ));
            assert!(matches!(
                parser.encode(Some(&document)),
                Err(UndoCodecError::Budget { .. })
            ));
            assert!(matches!(
                parser.encode_prepared("x", 1, &[], profile),
                Err(UndoCodecError::Budget { .. })
            ));
        } else {
            assert_eq!(parser.encode(Some(&document)).unwrap(), bytes);
            assert_eq!(parser.encode_prepared("x", 1, &[], profile).unwrap(), bytes);
            assert_eq!(parser.decode(&bytes).unwrap().unwrap(), document);
        }
    }
}
#[test]
fn nullable_bean_state_is_not_validated_as_an_application_transaction() {
    let bytes=br#"{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","branchId":-9223372036854775808,"xid":null,"sqlUndoLogs":["java.util.ArrayList",[null,{"@class":"org.apache.seata.rm.datasource.undo.SQLUndoLog","sqlType":null,"tableName":null,"beforeImage":null,"afterImage":null}]]}"#;
    let parser = JacksonUndoLogParser::default();
    let doc = parser.decode(bytes).unwrap().unwrap();
    assert_eq!(doc.branch_id(), i64::MIN);
    assert!(doc.sql_undo_logs().unwrap()[0].is_null());
    assert_eq!(
        parser
            .decode(&parser.encode(Some(&doc)).unwrap())
            .unwrap()
            .unwrap(),
        doc
    );
}
#[test]
fn duplicate_escaped_keys_unknown_classes_and_trailing_content_are_rejected() {
    let parser = JacksonUndoLogParser::default();
    for bytes in [
        br#"{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","xid":"one","xi\u0064":"two"}"#.as_slice(),
        br#"{"@class":"java.lang.ProcessBuilder"}"#,
        br#"{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog"} true"#,
        br#" { } "#,
        br#"{"$serde_json::private::Number":"1"}"#,
        br#"{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","branchId":{"$serde_json::private::Number":"1"}}"#,
    ] {assert!(parser.decode(bytes).is_err());}
}

#[test]
fn decimal_lexemes_and_unsigned_big_integer_never_pass_through_float() {
    let parser = JacksonUndoLogParser::default();
    let doc = parser.decode(NATIVE[3]).unwrap().unwrap();
    let logs = doc.sql_undo_logs().unwrap();
    let rows = &logs[0]["afterImage"]["rows"][1];
    let decimal = &rows[0]["fields"][1][1]["value"][1];
    assert_eq!(
        decimal.to_string(),
        "12345678901234567890123456789012345.123456789012345678901234567890"
    );
    let tiny = &rows[1]["fields"][1][1]["value"][1];
    assert_eq!(tiny.to_string(), "-1E-30");
    let unsigned = parser.decode(NATIVE[0]).unwrap().unwrap();
    assert_eq!(
        unsigned.sql_undo_logs().unwrap()[0]["afterImage"]["rows"][1][0]["fields"][1][10]["value"]
            [1]
        .to_string(),
        "18446744073709551615"
    );
}

#[test]
fn big_decimal_exponent_and_scale_follow_the_original_java_rejections() {
    let native = std::str::from_utf8(NATIVE[3]).unwrap();
    let original = "12345678901234567890123456789012345.123456789012345678901234567890";
    assert!(native.contains(original));
    let parser = JacksonUndoLogParser::default();
    for number in [
        "1E+2147483648",
        "1E-2147483648",
        "1.0E-2147483647",
        "0E+9999999999999999999999999999",
    ] {
        assert!(
            parser
                .decode(native.replace(original, number).as_bytes())
                .is_err(),
            "original Java rejected exponent/scale: {number}"
        );
    }
    for number in [
        "1E+2147483647",
        "1E-2147483647",
        "1.0E-2147483646",
        "1.0E+2147483647",
    ] {
        let bytes = native.replace(original, number);
        let document = parser.decode(bytes.as_bytes()).unwrap().unwrap();
        assert!(
            std::str::from_utf8(&parser.encode(Some(&document)).unwrap())
                .unwrap()
                .contains(number)
        );
    }
}

#[test]
fn node_scalar_and_output_budgets_are_independent_of_depth_and_input_length() {
    let bytes = br#"{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","xid":"short"}"#;
    let tiny_nodes =
        JacksonUndoLogParser::new(UndoCodecLimits::new(4096, 1, 32, 128, 1024).unwrap());
    assert_eq!(
        tiny_nodes.decode(bytes).unwrap_err(),
        UndoCodecError::Budget {
            limit: "JSON nodes/depth"
        }
    );
    let text = format!(
        r#"{{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","xid":"{}"}}"#,
        "x".repeat(129)
    );
    let tiny_scalar =
        JacksonUndoLogParser::new(UndoCodecLimits::new(4096, 100, 32, 128, 1024).unwrap());
    assert_eq!(
        tiny_scalar.decode(text.as_bytes()).unwrap_err(),
        UndoCodecError::Budget {
            limit: "scalar bytes"
        }
    );
    let doc = JacksonUndoLogParser::default()
        .decode(bytes)
        .unwrap()
        .unwrap();
    let tiny_output =
        JacksonUndoLogParser::new(UndoCodecLimits::new(10, 100, 32, 128, 1024).unwrap());
    assert_eq!(
        tiny_output.encode(Some(&doc)).unwrap_err(),
        UndoCodecError::Budget {
            limit: "encoded output"
        }
    );
}

#[test]
fn default_branch_obeys_structure_limits_while_its_input_remains_two_bytes() {
    let nodes = JacksonUndoLogParser::new(UndoCodecLimits::new(2, 1, 1, 128, 1024).unwrap());
    assert_eq!(
        nodes.decode(b"{}").unwrap_err(),
        UndoCodecError::Budget {
            limit: "JSON nodes/depth"
        }
    );
    let scalar = JacksonUndoLogParser::new(UndoCodecLimits::new(2, 100, 3, 1, 1).unwrap());
    assert_eq!(
        scalar.decode(b"{}").unwrap_err(),
        UndoCodecError::Budget {
            limit: "scalar bytes"
        }
    );
    let valid = JacksonUndoLogParser::new(UndoCodecLimits::new(2, 100, 3, 128, 1024).unwrap());
    let default = valid.decode(b"{}").unwrap().unwrap();
    assert_eq!(default.branch_id(), 0);
    assert_eq!(
        valid.encode(Some(&default)).unwrap_err(),
        UndoCodecError::Budget {
            limit: "encoded output"
        }
    );
}
#[test]
fn smaller_depth_and_node_limits_reject_before_a_document_is_published() {
    let parser = JacksonUndoLogParser::new(UndoCodecLimits::new(4096, 10, 3, 128, 1024).unwrap());
    assert!(matches!(
        parser.decode(NATIVE[0]),
        Err(UndoCodecError::Budget { .. })
    ));
    let deep = format!("{}0{}", "[".repeat(1000), "]".repeat(1000));
    assert!(matches!(
        JacksonUndoLogParser::default().decode(deep.as_bytes()),
        Err(UndoCodecError::Budget { .. })
    ));
}
#[test]
fn debug_and_errors_do_not_expose_document_values() {
    let parser = JacksonUndoLogParser::default();
    let doc=parser.decode(br#"{"@class":"org.apache.seata.rm.datasource.undo.BranchUndoLog","xid":"private-cell-123"}"#).unwrap().unwrap();
    assert!(!format!("{doc:?}").contains("private-cell"));
    let error = parser
        .decode(br#"{"@class":"private-cell-456"}"#)
        .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("private-cell"));
}
