//! Map.equals捕获NPE/CCE及其子类，仍须保留未捕获异常的具体类、消息和Arc身份。
use seata_sqlparser::{
    ColumnMeta, JavaObject, JavaObjectError, JavaString, MetadataError, MetadataLocale, TableMeta,
};
use std::{any::Any, error::Error, sync::Arc};

#[derive(Debug)]
struct ThrowingEquals(Arc<JavaObjectError>);
impl JavaObject for ThrowingEquals {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, _other: &dyn JavaObject) -> Result<bool, MetadataError> {
        Err(MetadataError::ObjectException(self.0.clone()))
    }
    fn java_hash_code(&self) -> Result<i32, MetadataError> {
        Ok(7)
    }
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(Some("throwing-equals".into()))
    }
}

fn tables(
    error: Arc<JavaObjectError>,
) -> Result<(TableMeta, TableMeta, Arc<ColumnMeta>, Arc<ColumnMeta>), MetadataError> {
    let locale = MetadataLocale::new("zh");
    let mine = TableMeta::new(locale.clone());
    let theirs = TableMeta::new(locale);
    let left = Arc::new(ColumnMeta::default());
    left.set_char_octet_length(Some(Arc::new(ThrowingEquals(error))))?;
    let right = Arc::new(ColumnMeta::default());
    right.set_char_octet_length(Some(Arc::new(1_i32)))?;
    mine.all_columns()
        .put(Some("c".into()), Some(left.clone()))?;
    theirs
        .all_columns()
        .put(Some("c".into()), Some(right.clone()))?;
    Ok((mine, theirs, left, right))
}

#[test]
fn map_equals_catches_both_exception_subclasses_after_direct_callback_retains_identity(
) -> Result<(), Box<dyn Error>> {
    for (class, parent) in [
        ("example.MyNpe", "java.lang.NullPointerException"),
        ("example.MyCce", "java.lang.ClassCastException"),
    ] {
        let error = Arc::new(
            JavaObjectError::new(class, Some(JavaString::from_utf16(vec![0xd800, 0, 0xdc00])))
                .with_superclasses([
                    parent,
                    "java.lang.RuntimeException",
                    "java.lang.Exception",
                    "java.lang.Throwable",
                ]),
        );
        let (mine, theirs, left, right) = tables(error.clone())?;
        let direct = right
            .java_equals(Some(left.as_ref()))
            .err()
            .ok_or("Missing direct callback exception")?;
        assert!(Arc::ptr_eq(
            direct.object_exception().ok_or("Missing identity")?,
            &error
        ));
        assert_eq!(direct.java_exception_class(), Some(class));
        assert_eq!(
            direct.java_message().map(JavaString::as_utf16),
            Some([0xd800, 0, 0xdc00].as_slice())
        );
        assert!(
            !mine.java_equals(Some(&theirs))?,
            "Map.equals must catch {class}"
        );
    }
    Ok(())
}

#[test]
fn map_equals_does_not_guess_catch_family_from_simple_name_or_swallow_other_subclasses(
) -> Result<(), Box<dyn Error>> {
    for (class, parent) in [
        ("example.MyCme", "java.util.ConcurrentModificationException"),
        (
            "example.NullPointerException",
            "java.lang.IllegalStateException",
        ),
    ] {
        let error = Arc::new(
            JavaObjectError::new(class, Some("uncaught".into()))
                .with_superclasses([parent, "java.lang.RuntimeException"]),
        );
        let (mine, theirs, _, _) = tables(error.clone())?;
        let actual = mine
            .java_equals(Some(&theirs))
            .err()
            .ok_or("Unrelated exception swallowed")?;
        assert!(Arc::ptr_eq(
            actual.object_exception().ok_or("Missing identity")?,
            &error
        ));
        assert_eq!(actual.java_exception_class(), Some(class));
    }
    Ok(())
}
