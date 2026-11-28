//! 动态Java Object方法的异常必须携原身份、类名和UTF16消息传播。
use seata_sqlparser::{
    ColumnList, ColumnMeta, JavaObject, JavaObjectError, JavaString, MetadataError,
};
use seata_sqlparser::{JavaThrowable, SQLParsingException};
use std::{any::Any, error::Error, sync::Arc};

#[derive(Debug)]
struct FailingValue(Arc<JavaObjectError>);
impl JavaObject for FailingValue {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, _other: &dyn JavaObject) -> Result<bool, MetadataError> {
        Err(MetadataError::ObjectException(Arc::clone(&self.0)))
    }
    fn java_hash_code(&self) -> Result<i32, MetadataError> {
        Err(MetadataError::ObjectException(Arc::clone(&self.0)))
    }
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Err(MetadataError::ObjectException(Arc::clone(&self.0)))
    }
}

impl std::fmt::Display for FailingValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("failing cause")
    }
}
impl Error for FailingValue {}
impl JavaThrowable for FailingValue {
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Err(MetadataError::ObjectException(Arc::clone(&self.0)))
    }
}

#[test]
fn throwing_cause_to_string_prevents_exception_construction_and_preserves_original_failure(
) -> Result<(), Box<dyn Error>> {
    let original = Arc::new(JavaObjectError::new(
        "java.lang.IllegalStateException",
        Some("cause failed".into()),
    ));
    let cause = Arc::new(FailingValue(Arc::clone(&original)));
    let error = SQLParsingException::from_java_cause(Some(cause))
        .err()
        .ok_or("Cause failure swallowed")?;
    assert!(Arc::ptr_eq(
        error.object_exception().ok_or("Missing original failure")?,
        &original
    ));
    Ok(())
}

#[test]
fn object_failures_preserve_identity_and_propagate_through_column_and_list(
) -> Result<(), Box<dyn Error>> {
    let original = Arc::new(JavaObjectError::new(
        "java.lang.IllegalStateException",
        Some(JavaString::from_utf16(vec![0xd800, 0, 0xdc00])),
    ));
    let mine = ColumnMeta::default();
    mine.set_char_octet_length(Some(Arc::new(1_i32)))?;
    let theirs = ColumnMeta::default();
    theirs.set_char_octet_length(Some(Arc::new(FailingValue(Arc::clone(&original)))))?;
    let errors = [
        mine.java_equals(Some(&theirs)).err(),
        theirs.java_hash_code().err(),
        theirs.java_to_string().err(),
        ColumnList::fixed(vec![Some(theirs.clone())])
            .java_to_string()
            .err(),
    ];
    for error in errors {
        let error = error.ok_or("Callback failure swallowed")?;
        assert_eq!(
            error.java_exception_class(),
            Some("java.lang.IllegalStateException")
        );
        let actual = error
            .object_exception()
            .ok_or("Original exception missing")?;
        assert!(Arc::ptr_eq(actual, &original));
        assert_eq!(
            actual.message_java().map(JavaString::as_utf16),
            Some([0xd800, 0, 0xdc00].as_slice())
        );
    }
    Ok(())
}
