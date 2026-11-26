//! Java 字符串与 Throwable 的 UTF-16 无损边界；不代替原76案例。
use seata_sqlparser::MetadataError;
use seata_sqlparser::{JavaString, JavaThrowable, SQLParsingException, SqlSequenceExpr};
use std::{error::Error, fmt, sync::Arc};

#[test]
fn java_string_preserves_all_code_units_and_java_hash() {
    let complete: Vec<u16> = (u16::MIN..=u16::MAX).collect();
    assert_eq!(
        JavaString::from_utf16(complete.clone()).as_utf16(),
        complete
    );
    for units in [
        vec![],
        vec![0],
        vec![0xd800],
        vec![0xdc00],
        vec![0xd800, 0xdc00],
        vec![0x61, 0xd800, 0x62],
        vec![0xdc00, 0xd800],
    ] {
        let value = JavaString::from_utf16(units.clone());
        assert_eq!(value.as_utf16(), units);
        let hash = units.iter().fold(0_i32, |sum, unit| {
            sum.wrapping_mul(31).wrapping_add(i32::from(*unit))
        });
        assert_eq!(value.java_hash_code(), hash);
        assert_eq!(value.as_str().is_some(), String::from_utf16(&units).is_ok());
    }
    assert_ne!(
        JavaString::from_utf16(vec![0xd800]),
        JavaString::from_utf16(vec![0xfffd])
    );
    assert_ne!(
        JavaString::from_utf16(vec![0xd800]),
        JavaString::from_utf16(vec![0xdc00])
    );
}

#[test]
fn java_string_diagnostic_does_not_silently_replace_surrogates() {
    assert_eq!(
        JavaString::from_utf16(vec![0x61, 0xd800, 0x62]).to_string(),
        "a\\ud800b"
    );
    assert_eq!(
        JavaString::from_utf16(vec![0xd800, 0xdc00]).as_str(),
        Some("\u{10000}")
    );
}

#[test]
fn sequence_raw_getters_and_mutations_retain_unpaired_surrogates() {
    let mut expr = SqlSequenceExpr::from_java(Some(JavaString::from_utf16(vec![0xd800])), None);
    assert_eq!(
        expr.sequence_java().map(JavaString::as_utf16),
        Some([0xd800].as_slice())
    );
    assert!(expr.function_java().is_none());
    expr.set_function_java(Some(JavaString::from_utf16(vec![0xdc00])));
    assert_eq!(
        expr.function_java().map(JavaString::as_utf16),
        Some([0xdc00].as_slice())
    );
    expr.set_sequence_java(None);
    assert!(expr.sequence_java().is_none());
    expr.set_function(Some("newFunction".into()));
    assert_eq!(expr.function(), Some("newFunction"));
    assert_eq!(
        expr.function_java().map(JavaString::as_utf16),
        Some("newFunction".encode_utf16().collect::<Vec<_>>().as_slice())
    );
}

#[test]
fn exception_raw_message_and_to_string_retain_unpaired_surrogates() {
    let raw = JavaString::from_utf16(vec![0xd800, 0, 0xdc00]);
    let error = SQLParsingException::with_java_message(Some(raw.clone()), None);
    assert_eq!(error.message_java(), Some(&raw));
    let mut expected: Vec<u16> = "org.apache.seata.sqlparser.SQLParsingException: "
        .encode_utf16()
        .collect();
    expected.extend(raw.as_utf16());
    assert_eq!(error.java_to_string().as_utf16(), expected);
    assert!(error.source().is_none());
}

#[derive(Debug)]
struct RawCause(JavaString);
impl fmt::Display for RawCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
impl Error for RawCause {}
impl JavaThrowable for RawCause {
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(Some(self.0.clone()))
    }
}

#[test]
fn exception_raw_cause_retains_text_and_original_error_identity() -> Result<(), Box<dyn Error>> {
    let raw = JavaString::from_utf16(vec![0xdc00, 0xd800]);
    let cause = Arc::new(RawCause(raw.clone()));
    let error = SQLParsingException::from_java_cause(Some(Arc::clone(&cause)))?;
    assert_eq!(error.message_java(), Some(&raw));
    let erased: Arc<dyn Error + Send + Sync> = cause;
    assert!(Arc::ptr_eq(error.cause().ok_or("Missing cause")?, &erased));
    assert!(error.source().ok_or("Missing source")?.is::<RawCause>());
    let absent = SQLParsingException::from_java_cause::<RawCause>(None)?;
    assert!(absent.message_java().is_none());
    assert!(absent.cause().is_none());
    Ok(())
}
