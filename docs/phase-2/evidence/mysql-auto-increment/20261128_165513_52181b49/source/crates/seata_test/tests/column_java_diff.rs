//! 固定原Java导出的21项补充观察：完整码元域、空原因文本及可重入对象回调。
use seata_sqlparser::MetadataError;
#[path = "support/callback_object.rs"]
mod callback_object;
use callback_object::{CallbackKind, CallbackObject};
use seata_sqlparser::{
    ColumnMeta, JavaString, JavaThrowable, SQLParsingException, SqlSequenceExpr,
};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    io::Write,
    sync::{Arc, Mutex},
};
type TestResult = Result<(), Box<dyn Error>>;
static OUTPUT: Mutex<()> = Mutex::new(());

fn units(value: &JavaString) -> String {
    let mut text = String::new();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for unit in value.as_utf16() {
        for byte in unit.to_be_bytes() {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    text
}

fn optional_units(value: Option<&JavaString>) -> String {
    value.map_or_else(|| "null".into(), units)
}

fn check_edge(key: &str, actual: &[String]) -> TestResult {
    let rows: Vec<_> = include_str!("fixtures/sqlparser_core/column-edges-java-v2.tsv")
        .lines()
        .collect();
    let expected: BTreeMap<_, Vec<_>> = rows
        .iter()
        .map(|row| {
            let mut fields = row.split('\t');
            (fields.next().unwrap_or_default(), fields.collect())
        })
        .collect();
    assert_eq!(rows.len(), 21);
    assert_eq!(expected.len(), rows.len());
    assert_eq!(
        actual.iter().map(String::as_str).collect::<Vec<_>>(),
        *expected.get(key).ok_or("Missing Java edge key")?,
        "Java mismatch: {key}"
    );
    if let Some(path) = std::env::var_os("SEATA_COLUMN_EDGE_OBSERVATIONS") {
        let _guard = OUTPUT
            .lock()
            .map_err(|_| "Edge observation lock poisoned")?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{key}\t{}", actual.join("\t"))?;
    }
    Ok(())
}

#[test]
fn exhaustive_yes_case_equivalence_matches_original_jdk17() -> TestResult {
    let column = ColumnMeta::default();
    for position in 0..3 {
        let mut accepted = Vec::new();
        for code in u16::MIN..=u16::MAX {
            let mut candidate = vec![0x59, 0x45, 0x53];
            candidate[position] = code;
            column.set_is_autoincrement(Some(JavaString::from_utf16(candidate)))?;
            if column.is_autoincrement()? {
                accepted.push(format!("{code:04x}"));
            }
        }
        check_edge(
            &format!("yes-ignore-case-{position}"),
            &[accepted.join(",")],
        )?;
    }
    Ok(())
}

#[test]
fn original_java_string_hash_and_all_code_units_match() -> TestResult {
    let vectors = [
        vec![],
        vec![0],
        vec![0xd800],
        vec![0xdc00],
        vec![0xd800, 0xdc00],
        vec![0x61, 0xd800, 0x62],
        vec![0xdc00, 0xd800],
    ];
    for (index, vector) in vectors.into_iter().enumerate() {
        let value = JavaString::from_utf16(vector);
        check_edge(
            &format!("utf16-{index}"),
            &[units(&value), value.java_hash_code().to_string()],
        )?;
    }
    let complete = JavaString::from_utf16((u16::MIN..=u16::MAX).collect());
    check_edge(
        "all-utf16-units",
        &[units(&complete), complete.java_hash_code().to_string()],
    )
}

#[derive(Debug)]
struct JavaTextCause(Option<JavaString>);
impl fmt::Display for JavaTextCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(value) => fmt::Display::fmt(value, f),
            None => f.write_str("null Java text"),
        }
    }
}
impl Error for JavaTextCause {}
impl JavaThrowable for JavaTextCause {
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(self.0.clone())
    }
}

#[test]
fn raw_sequence_exception_and_nullable_cause_text_match_original_java() -> TestResult {
    let raw = JavaString::from_utf16(vec![0xd800, 0, 0xdc00]);
    let mut sequence = SqlSequenceExpr::from_java(Some(raw.clone()), None);
    check_edge(
        "sequence-raw-initial",
        &[
            optional_units(sequence.sequence_java()),
            optional_units(sequence.function_java()),
        ],
    )?;
    sequence.set_function_java(Some(JavaString::from_utf16(vec![0xdc00])));
    sequence.set_sequence_java(None);
    check_edge(
        "sequence-raw-mutated",
        &[
            optional_units(sequence.sequence_java()),
            optional_units(sequence.function_java()),
        ],
    )?;
    let error = SQLParsingException::with_java_message(Some(raw.clone()), None);
    check_edge(
        "exception-raw",
        &[
            optional_units(error.message_java()),
            units(&error.java_to_string()),
            error.cause().is_none().to_string(),
        ],
    )?;
    let mut cause_text: Vec<u16> = "java.lang.RuntimeException: ".encode_utf16().collect();
    cause_text.extend(raw.as_utf16());
    let cause = Arc::new(JavaTextCause(Some(JavaString::from_utf16(cause_text))));
    let error = SQLParsingException::from_java_cause(Some(Arc::clone(&cause)))?;
    let erased: Arc<dyn Error + Send + Sync> = cause;
    check_edge(
        "exception-raw-cause",
        &[
            optional_units(error.message_java()),
            units(&error.java_to_string()),
            Arc::ptr_eq(error.cause().ok_or("Missing cause")?, &erased).to_string(),
        ],
    )?;
    let cause = Arc::new(JavaTextCause(None));
    let error = SQLParsingException::from_java_cause(Some(Arc::clone(&cause)))?;
    let erased: Arc<dyn Error + Send + Sync> = cause;
    check_edge(
        "exception-null-cause-text",
        &[
            optional_units(error.message_java()),
            Arc::ptr_eq(error.cause().ok_or("Missing cause")?, &erased).to_string(),
        ],
    )
}

#[test]
fn column_default_and_raw_overflow_match_original_java() -> TestResult {
    let column = ColumnMeta::default();
    check_edge(
        "column-default",
        &[
            column.java_hash_code()?.to_string(),
            units(&column.java_to_string()?),
        ],
    )?;
    column.set_data_type(i32::MAX)?;
    column.set_column_size(i32::MAX)?;
    column.set_column_name(Some(JavaString::from_utf16(vec![0xd800, 0, 0xdc00])))?;
    check_edge(
        "column-raw-overflow",
        &[
            column.java_hash_code()?.to_string(),
            units(&column.java_to_string()?),
        ],
    )
}

#[test]
fn object_callback_read_order_matches_original_java() -> TestResult {
    let left = ColumnMeta::default();
    let right = ColumnMeta::default();
    right.set_ordinal_position(7)?;
    left.set_char_octet_length(Some(Arc::new(CallbackObject {
        column: left.clone(),
        kind: CallbackKind::Equals,
    })))?;
    right.set_char_octet_length(Some(Arc::new(CallbackObject {
        column: left.clone(),
        kind: CallbackKind::Equals,
    })))?;
    check_edge(
        "column-callback-equals",
        &[
            left.java_equals(Some(&right))?.to_string(),
            left.ordinal_position()?.to_string(),
        ],
    )?;
    left.set_char_octet_length(None)?;
    right.set_char_octet_length(None)?;
    let column = ColumnMeta::default();
    column.set_char_octet_length(Some(Arc::new(CallbackObject {
        column: column.clone(),
        kind: CallbackKind::Hash,
    })))?;
    let hash = column.java_hash_code()?;
    check_edge(
        "column-callback-hash",
        &[hash.to_string(), column.ordinal_position()?.to_string()],
    )?;
    column.set_char_octet_length(None)?;
    column.set_ordinal_position(0)?;
    column.set_char_octet_length(Some(Arc::new(CallbackObject {
        column: column.clone(),
        kind: CallbackKind::Text,
    })))?;
    let text = column.java_to_string()?;
    check_edge(
        "column-callback-text",
        &[units(&text), column.ordinal_position()?.to_string()],
    )?;
    column.set_char_octet_length(None)?;
    Ok(())
}
