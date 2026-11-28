//! 固定 Java Fastjson Undo 字节与 MySQL 原生物理镜像绑定。
#![allow(clippy::unwrap_used)]
use seata_at::{FastjsonUndoLogParser, MySqlJdbcProfile, SqlType, UndoCodecLimits};
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, ColumnType, DecimalValue, FieldValue, MySqlType,
    TableMeta,
};
const FIXTURE: &[u8] = include_bytes!("fixtures/fastjson_native_wire.txt");
const PROFILE: MySqlJdbcProfile = MySqlJdbcProfile::Connector8NumericTinyInt;
const XID: &str = "fastjson-native:42";

fn schema() -> TableMeta {
    schema_with_decimal(20, 10)
}

fn schema_with_decimal(precision: u32, scale: u32) -> TableMeta {
    let columns = [
        (
            "id",
            ColumnType::SignedInteger,
            MySqlType::BigInt,
            None,
            None,
        ),
        (
            "small",
            ColumnType::SignedInteger,
            MySqlType::Int,
            None,
            None,
        ),
        (
            "wide",
            ColumnType::SignedInteger,
            MySqlType::BigInt,
            None,
            None,
        ),
        (
            "unsigned",
            ColumnType::UnsignedInteger,
            MySqlType::BigInt,
            None,
            None,
        ),
        (
            "decimal",
            ColumnType::Decimal,
            MySqlType::Decimal,
            None,
            Some(scale),
        ),
        (
            "whole",
            ColumnType::Decimal,
            MySqlType::Decimal,
            None,
            Some(0),
        ),
        (
            "text",
            ColumnType::Text,
            MySqlType::VarChar,
            Some(128),
            None,
        ),
        (
            "bytes",
            ColumnType::Binary,
            MySqlType::VarBinary,
            Some(128),
            None,
        ),
        (
            "nullable",
            ColumnType::Text,
            MySqlType::VarChar,
            Some(128),
            None,
        ),
    ];
    let columns = columns
        .into_iter()
        .enumerate()
        .map(|(index, (name, kind, native, length, scale))| {
            let domain = match kind {
                ColumnType::LocalDate => ColumnDomain::LocalDate,
                ColumnType::InternalTime => ColumnDomain::InternalTime {
                    fractional_seconds: 6,
                },
                ColumnType::Timestamp => ColumnDomain::Timestamp {
                    fractional_seconds: 6,
                },
                ColumnType::LocalDateTime => ColumnDomain::LocalDateTime {
                    fractional_seconds: 6,
                },
                ColumnType::SignedInteger | ColumnType::UnsignedInteger => ColumnDomain::Integer {
                    bits: if name == "small" { 32 } else { 64 },
                    unsigned: kind == ColumnType::UnsignedInteger,
                },
                ColumnType::Text => ColumnDomain::Text {
                    charset: CharacterSet::Utf8Mb4,
                    character_limit: Some(32),
                    fixed_width: false,
                },
                ColumnType::Binary => ColumnDomain::Binary { fixed_width: false },
                ColumnType::Decimal => ColumnDomain::Decimal { unsigned: false },
            };
            ColumnMeta::new(
                name.into(),
                kind,
                name == "nullable",
                index as u32 + 1,
                (index == 0).then_some(1),
                length,
                scale.map(|_| if name == "decimal" { precision } else { 20 }),
                scale,
            )
            .unwrap()
            .with_domain(domain)
            .unwrap()
            .with_mysql_type(native)
            .unwrap()
        })
        .collect();
    TableMeta::new("undo_fixture".into(), columns).unwrap()
}

#[test]
fn original_fastjson_restores_complete_ordered_insert_update_delete() {
    let parser = FastjsonUndoLogParser::default();
    let logs = parser
        .restore_prepared(FIXTURE, XID, 42, &[schema()], PROFILE)
        .unwrap();
    assert_eq!(
        logs.iter().map(|log| log.sql_type()).collect::<Vec<_>>(),
        [SqlType::Insert, SqlType::Update, SqlType::Delete]
    );
    let mut observations = Vec::new();
    let codes = [-5, 4, -5, -5, 3, 3, 12, -3, 12];
    for log in &logs {
        let operation = match log.sql_type() {
            SqlType::Insert => "INSERT",
            SqlType::Update => "UPDATE",
            SqlType::Delete => "DELETE",
        };
        observations.push(format!("LOG\t{operation}\t{}", log.table_name()));
        for image in [log.before_image(), log.after_image()] {
            observations.push(format!("IMAGE\t{}", image.rows().len()));
            for row in image.rows() {
                for (field, code) in row.fields().iter().zip(codes) {
                    let hex = |bytes: &[u8]| {
                        use std::fmt::Write;
                        bytes.iter().fold(String::new(), |mut text, byte| {
                            write!(text, "{byte:02x}").unwrap();
                            text
                        })
                    };
                    let canonical = match field.value() {
                        FieldValue::LocalDate(value) => value.to_sql_string(),
                        FieldValue::InternalTime(value) => value.to_sql_string(6).unwrap(),
                        FieldValue::Timestamp(value) => value.to_java_utc_string(),
                        FieldValue::LocalDateTime(value) => value.to_java_string(),
                        FieldValue::Null => "null".to_owned(),
                        FieldValue::Signed(value) => value.to_string(),
                        FieldValue::Unsigned(value) => value.to_string(),
                        FieldValue::Decimal(value) => value.as_str().to_owned(),
                        FieldValue::Text(value) => hex(value.as_bytes()),
                        FieldValue::Binary(value) => hex(value),
                    };
                    let key = if field.is_primary_key() {
                        "PRIMARY_KEY"
                    } else {
                        "NULL"
                    };
                    observations.push(format!(
                        "FIELD\t{}\t{code}\t{key}\t{canonical}",
                        field.name()
                    ));
                }
            }
        }
    }
    for line in observations {
        println!("SEATA_FASTJSON_NATIVE\t{line}");
    }
    assert!(logs[0].before_image().rows().is_empty());
    assert!(logs[2].after_image().rows().is_empty());
    assert_eq!(logs[0].after_image(), logs[1].before_image());
    assert_eq!(logs[1].after_image(), logs[2].before_image());
    let row = &logs[1].after_image().rows()[0];
    let expected = [
        FieldValue::Signed(3),
        FieldValue::Signed(-7),
        FieldValue::Signed(i64::MIN),
        FieldValue::Unsigned(u64::MAX),
        FieldValue::Decimal(DecimalValue::new("-0.0000001200").unwrap()),
        FieldValue::Decimal(DecimalValue::new("42").unwrap()),
        FieldValue::Text("update-螃蟹".into()),
        FieldValue::Binary(vec![0, 255, 127]),
        FieldValue::Null,
    ];
    assert_eq!(
        row.fields()
            .iter()
            .map(|field| field.value().clone())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn fastjson_rejects_malformed_or_unbound_native_values_before_returning_logs() {
    let parser = FastjsonUndoLogParser::default();
    let schema = [schema()];
    let fixture = std::str::from_utf8(FIXTURE).unwrap();
    for (before, after) in [
        (r#""branchId":42"#, r#""branchId":42,"branchId":42"#),
        (r#""branchId":42"#, r#""branchId":43"#),
        ("x'00FF7F'", "x'0F7'"),
        ("x'00FF7F'", r#""AP9/""#),
        ("x'00FF7F'", r#"{"\u0040seata.fast.binary":"AP9/"}"#),
        ("3L", "3LL"),
        (r#""type":-5,"value":3L"#, r#""type":4,"value":3L"#),
        (r#""name":"nullable""#, r#""name":"unmapped""#),
        (r#""rows":[]"#, r#""rows":[],"extra":true"#),
        (r#""value":3L"#, r#""value":{"$ref":"@"}"#),
    ] {
        assert!(fixture.contains(before), "{before}");
        let broken = fixture.replacen(before, after, 1);
        assert!(
            parser
                .restore_prepared(broken.as_bytes(), XID, 42, &schema, PROFILE)
                .is_err(),
            "{before}"
        );
    }
    let budget = UndoCodecLimits::new(FIXTURE.len() - 1, 1024, 32, 512, 4096).unwrap();
    assert!(FastjsonUndoLogParser::new(budget)
        .restore_prepared(FIXTURE, XID, 42, &schema, PROFILE)
        .is_err());
    assert!(parser
        .restore_prepared(FIXTURE, "wrong", 42, &schema, PROFILE)
        .is_err());
}

#[test]
fn fastjson_preflight_authorizes_table_lookup_before_physical_binding() {
    let parser = FastjsonUndoLogParser::default();
    let decoded = parser.decode_for_restore(FIXTURE, XID, 42).unwrap();
    assert_eq!(decoded.table_names().collect::<Vec<_>>(), ["undo_fixture"]);
    let fixture = std::str::from_utf8(FIXTURE).unwrap();
    let with_two_tables = fixture.replace("undo_fixture", "second_fixture");
    let first_undo = with_two_tables.replacen("second_fixture", "first_fixture", 3);
    let two = parser
        .decode_for_restore(first_undo.as_bytes(), XID, 42)
        .unwrap();
    assert_eq!(
        two.table_names().collect::<Vec<_>>(),
        ["first_fixture", "second_fixture"]
    );
    assert!(parser
        .restore_document(&decoded, "wrong", 42, &[schema()], PROFILE)
        .is_err());
    assert!(parser
        .restore_document(&decoded, XID, 42, &[], PROFILE)
        .is_err());
    let smaller = UndoCodecLimits::new(FIXTURE.len() - 1, 1024, 32, 512, 4096).unwrap();
    assert!(FastjsonUndoLogParser::new(smaller)
        .restore_document(&decoded, XID, 42, &[schema()], PROFILE)
        .is_err());
    let normalized_cap =
        UndoCodecLimits::new(FIXTURE.len(), 262_144, 32, 1_398_104, 25_165_824).unwrap();
    let parser_with_wire_only_budget = FastjsonUndoLogParser::new(normalized_cap);
    assert!(parser_with_wire_only_budget
        .restore_prepared(FIXTURE, XID, 42, &[schema()], PROFILE)
        .is_err());
    assert!(parser_with_wire_only_budget
        .restore_document(&decoded, XID, 42, &[schema()], PROFILE)
        .is_err());
    let direct = parser
        .restore_prepared(FIXTURE, XID, 42, &[schema()], PROFILE)
        .unwrap();
    let split = parser
        .restore_document(&decoded, XID, 42, &[schema()], PROFILE)
        .unwrap();
    assert_eq!(direct, split);
}

#[test]
fn fastjson_preflight_rejects_unbound_identity_and_unqueryable_tables() {
    let parser = FastjsonUndoLogParser::default();
    let fixture = std::str::from_utf8(FIXTURE).unwrap();
    assert!(parser.decode_for_restore(FIXTURE, "wrong", 42).is_err());
    assert!(parser.decode_for_restore(FIXTURE, XID, 0).is_err());
    for edited in [
        fixture.replacen(r#","sqlType":"INSERT""#, "", 1),
        fixture.replacen(r#""name":"id","#, "", 1),
        fixture.replacen(r#""keyType":"PRIMARY_KEY","#, "", 1),
        fixture.replacen(r#""type":-5,"#, "", 1),
        fixture.replacen(r#""name":"small""#, r#""name":"id""#, 1),
        fixture.replacen(r#""name":"small""#, r#""name":"ID""#, 1),
        fixture.replacen(r#""keyType":"PRIMARY_KEY""#, r#""keyType":"NULL""#, 1),
        fixture.replacen(r#""value":3L"#, r#""value":null"#, 1),
        fixture.replace("undo_fixture", "db.undo_fixture"),
        fixture.replacen(r#""tableName":"undo_fixture""#, r#""tableName":"other""#, 1),
        fixture.replacen(r#""sqlType":"INSERT""#, r#""sqlType":"DELETE""#, 1),
        fixture.replacen(r#""sqlType":"INSERT""#, r#""sqlType":"SELECT""#, 1),
        fixture.replacen(r#""value":3L"#, r#""value":{"$ref":"@"}"#, 1),
        fixture.replacen(r#""value":3L"#, r#""value":[]"#, 1),
    ] {
        assert!(parser
            .decode_for_restore(edited.as_bytes(), XID, 42)
            .is_err());
    }
    assert!(parser.decode_for_restore(br#"{"@type":"org.apache.seata.rm.datasource.undo.BranchUndoLog","branchId":42,"xid":"fastjson-native:42","sqlUndoLogs":[]}"#, XID, 42).is_err());
}

#[test]
fn fastjson_source_test_decode_default_content() {
    let parser = FastjsonUndoLogParser::default();
    let bytes = parser.default_content();
    let branch = parser.decode_branch(bytes).unwrap();
    assert!(branch.is_some());
    let branch = branch.unwrap();
    let null_xid = branch.xid().is_none();
    assert!(null_xid);
    let branch_id = branch.branch_id();
    assert_eq!(branch_id, 0);
    let null_logs = branch.sql_undo_log_count().is_none();
    assert!(null_logs);
    assert_eq!(bytes, b"{}");
    let wire = std::str::from_utf8(bytes).unwrap();
    println!("SEATA_FASTJSON_DEFAULT\t{wire}\ttrue\t{null_xid}\t{branch_id}\t{null_logs}");
}

#[test]
fn fastjson_branch_envelope_keeps_null_and_empty_logs_distinct() {
    let parser = FastjsonUndoLogParser::default();
    assert!(parser.decode_branch(b"null").unwrap().is_none());
    let empty = parser
        .decode_branch(br#"{"sqlUndoLogs":[]}"#)
        .unwrap()
        .unwrap();
    assert_eq!(empty.sql_undo_log_count(), Some(0));
    let original = parser.decode_branch(FIXTURE).unwrap().unwrap();
    assert_eq!(original.xid(), Some(XID));
    assert_eq!(original.branch_id(), 42);
    assert_eq!(original.sql_undo_log_count(), Some(3));
    assert!(parser.decode_branch(br#"{"sqlUndoLogs":{}}"#).is_err());
    assert!(parser.decode_branch(br#"{"@type":"unexpected"}"#).is_err());
    assert!(parser
        .decode_for_restore(parser.default_content(), XID, 42)
        .is_err());
}

#[test]
fn fastjson_source_test_write_class_name() {
    use seata_at::{
        FastjsonSqlUndoLog, FastjsonUndoField, FastjsonUndoFieldValue, FastjsonUndoRecords,
    };
    let parser = FastjsonUndoLogParser::default();
    let after = FastjsonUndoRecords::new(
        Some("t1".into()),
        vec![vec![
            FastjsonUndoField::new("id".into(), -5, true, FastjsonUndoFieldValue::Long(0)),
            FastjsonUndoField::new(
                "money".into(),
                3,
                false,
                FastjsonUndoFieldValue::Decimal(DecimalValue::new("1").unwrap()),
            ),
        ]],
    );
    let log = FastjsonSqlUndoLog::new(
        SqlType::Insert,
        Some("table_name".into()),
        FastjsonUndoRecords::new(None, Vec::new()),
        after,
    );
    let logs = [log];
    let bytes = parser
        .encode_branch(Some("xiddddddddddd"), 123456, Some(&logs))
        .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let class_tag = text.contains("\"@type\"");
    assert!(class_tag);
    let branch = parser.decode_branch(text.as_bytes()).unwrap().unwrap();
    let long = branch.field_value_class(0, true, 0, 0) == Some("java.lang.Long");
    let decimal = branch.field_value_class(0, true, 0, 1) == Some("java.math.BigDecimal");
    assert!(long);
    assert!(decimal);
    assert!(parser
        .decode_for_restore(&bytes, "xiddddddddddd", 123456)
        .is_err());
    let cap = UndoCodecLimits::new(bytes.len() - 1, 262_144, 32, 1_398_104, 25_165_824).unwrap();
    assert!(matches!(
        FastjsonUndoLogParser::new(cap).encode_branch(Some("xiddddddddddd"), 123456, Some(&logs)),
        Err(seata_at::UndoCodecError::Budget {
            limit: "encoded output"
        })
    ));
    println!("SEATA_FASTJSON_WRITE_CLASS\t{class_tag}\t{long}\t{decimal}\t{text}");
}

#[test]
fn fastjson_string_escapes_match_original_writer() {
    let input =
        "\0\u{1}\u{8}\t\n\u{b}\u{c}\r\u{e}\u{1f} \"/\\~\u{7f}\u{80}\u{9f}\u{a0}\u{2028}\u{2029}蟹";
    let bytes = FastjsonUndoLogParser::default()
        .encode_branch(Some(input), 42, None)
        .unwrap();
    let wire = std::str::from_utf8(&bytes).unwrap();
    let (_, encoded) = wire
        .strip_suffix('}')
        .unwrap()
        .rsplit_once(",\"xid\":")
        .unwrap();
    let expected = concat!(
        r#""\u0000\u0001\b\t\n\u000B\f\r\u000E\u001F \"/\\~\u007F\u0080\u009F"#,
        "\u{a0}",
        r#"\u2028\u2029蟹""#,
    );
    assert_eq!(encoded, expected);
    println!("SEATA_FASTJSON_ESCAPE\t{encoded}");
    let fixture = std::str::from_utf8(FIXTURE)
        .unwrap()
        .replacen("3L", "9223372036854775808L", 1);
    let branch = FastjsonUndoLogParser::default()
        .decode_branch(fixture.as_bytes())
        .unwrap()
        .unwrap();
    let class = branch.field_value_class(0, true, 0, 0).unwrap();
    assert_eq!(class, "java.math.BigInteger");
    println!("SEATA_FASTJSON_LONG_OVERFLOW\t{class}");
}

#[path = "support/fastjson_serial_array.rs"]
mod serial_array_source;

#[path = "support/fastjson_h2_source.rs"]
mod h2_source;

const FASTJSON2: &[u8] = include_bytes!("fixtures/fastjson2_native_wire.bin");

#[test]
fn original_fastjson2_keeps_java_physical_value_classes_and_wire_bytes() {
    let parser = seata_at::Fastjson2UndoLogParser::default();
    let branch = parser.decode_branch(FASTJSON2).unwrap();
    assert_eq!(branch.xid(), Some(XID));
    assert_eq!(branch.branch_id(), 42);
    let fields = &branch.logs().unwrap()[1].after_image().unwrap().rows()[0];
    assert_eq!(
        fields
            .iter()
            .map(|field| field.value().java_class())
            .collect::<Vec<_>>(),
        [
            "java.lang.Long",
            "java.lang.Integer",
            "java.lang.Long",
            "java.math.BigInteger",
            "java.math.BigDecimal",
            "java.math.BigDecimal",
            "java.lang.String",
            "[B",
            "null",
        ]
    );
    assert_eq!(
        parser
            .encode_branch(branch.xid(), branch.branch_id(), branch.logs())
            .unwrap(),
        FASTJSON2
    );
}

#[test]
fn original_fastjson2_restores_complete_ordered_mysql_images() {
    let expected = FastjsonUndoLogParser::default()
        .restore_prepared(FIXTURE, XID, 42, &[schema()], PROFILE)
        .unwrap();
    let parser = seata_at::Fastjson2UndoLogParser::default();
    let document = parser.decode_for_restore(FASTJSON2, XID, 42).unwrap();
    assert_eq!(document.table_names().collect::<Vec<_>>(), ["undo_fixture"]);
    let logs = document.bind(XID, 42, &[schema()], PROFILE).unwrap();
    assert!(logs == expected);
    assert!(matches!(
        document.bind(XID, 43, &[schema()], PROFILE),
        Err(seata_at::UndoCodecError::Shape {
            reason: "JSONB restored original branch identity"
        })
    ));
}

#[test]
fn original_fastjson2_rejects_wrong_identity_or_incomplete_metadata() {
    let parser = seata_at::Fastjson2UndoLogParser::default();
    for (xid, branch, tables) in [
        ("other:42", 42, vec![schema()]),
        (XID, 41, vec![schema()]),
        (XID, 42, vec![]),
        (XID, 42, vec![schema(), schema()]),
    ] {
        assert!(parser
            .restore_prepared(FASTJSON2, xid, branch, &tables, PROFILE)
            .is_err());
    }
}

#[test]
fn original_fastjson2_preflights_every_log_and_rejects_wrong_boxed_class() {
    use seata_at::{
        FastjsonUndoFieldValue, JacksonSqlUndoLog, JacksonUndoField, JacksonUndoFieldValue,
        JacksonUndoRecords,
    };

    let parser = seata_at::Fastjson2UndoLogParser::default();
    let branch = parser.decode_branch(FASTJSON2).unwrap();
    let mut logs = branch.logs().unwrap().to_vec();
    let after = logs[1].after_image().unwrap();
    let mut rows = after.rows().to_vec();
    // 数值 3 可以放入 BIGINT，但 Java Connector/J 为该列写入的是 Long。
    rows[0][0] = JacksonUndoField::new(
        "id".into(),
        -5,
        true,
        JacksonUndoFieldValue::Common(FastjsonUndoFieldValue::Integer(3)),
    );
    logs[1] = JacksonSqlUndoLog::new(
        Some("UPDATE".into()),
        Some("undo_fixture".into()),
        logs[1].before_image().cloned(),
        Some(JacksonUndoRecords::new(Some("undo_fixture".into()), rows)),
    );
    let wrong_class = parser.encode_branch(branch.xid(), 42, Some(&logs)).unwrap();
    assert!(parser
        .restore_prepared(&wrong_class, XID, 42, &[schema()], PROFILE)
        .is_err());

    logs.clone_from_slice(branch.logs().unwrap());
    logs[2] = JacksonSqlUndoLog::new(
        Some("DELETE".into()),
        Some("other_table".into()),
        logs[2].before_image().cloned(),
        logs[2].after_image().cloned(),
    );
    let wrong_table = parser.encode_branch(branch.xid(), 42, Some(&logs)).unwrap();
    assert!(parser
        .restore_prepared(&wrong_table, XID, 42, &[schema()], PROFILE)
        .is_err());
}

#[test]
fn original_fastjson2_restores_max_precision_decimal_and_small_biginteger() {
    let bytes = include_bytes!("fixtures/fastjson2_native_wide.bin");
    let parser = seata_at::Fastjson2UndoLogParser::default();
    let branch = parser.decode_branch(bytes).unwrap();
    assert_eq!(
        parser
            .encode_branch(branch.xid(), branch.branch_id(), branch.logs())
            .unwrap(),
        bytes
    );
    let logs = parser
        .restore_prepared(bytes, XID, 42, &[schema_with_decimal(65, 30)], PROFILE)
        .unwrap();
    let fields = logs[1].after_image().rows()[0].fields();
    assert_eq!(fields[3].value(), &FieldValue::Unsigned(1));
    assert_eq!(
        fields[4].value(),
        &FieldValue::Decimal(
            DecimalValue::new(
                "-12345678901234567890123456789012345.678901234567890123456789012300"
            )
            .unwrap()
        )
    );
    assert!(parser
        .restore_prepared(bytes, XID, 42, &[schema()], PROFILE)
        .is_err());
}

#[test]
fn original_fastjson2_rejects_invalid_decimal_scale_and_binary_length() {
    let parser = seata_at::Fastjson2UndoLogParser::default();
    for (prefix, replacement) in [
        (&[0xb9, 0x0a, 0x33, 0x50][..], 0x1f),
        (&[0x91, 0x03, 0x00, 0xff, 0x7f][..], 0xff),
    ] {
        let mut malformed = FASTJSON2.to_vec();
        let offset = malformed
            .windows(prefix.len())
            .position(|part| part == prefix)
            .unwrap();
        malformed[offset + 1] = replacement;
        assert!(parser.decode_branch(&malformed).is_err());
    }
}

#[test]
fn original_fastjson2_restores_under_the_exact_original_byte_budget() {
    let parser = seata_at::Fastjson2UndoLogParser::new(
        UndoCodecLimits::new(FASTJSON2.len(), 4096, 32, 4096, 16384).unwrap(),
    );
    assert!(parser.decode_branch(FASTJSON2).is_ok());
    assert!(parser
        .restore_prepared(FASTJSON2, XID, 42, &[schema()], PROFILE)
        .is_ok());
}
