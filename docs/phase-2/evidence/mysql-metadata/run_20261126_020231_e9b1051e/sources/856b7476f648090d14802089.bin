//! nullable、引用、列表模式及可重入比较的Rust责任；不计为原16案例。
use seata_sqlparser::MetadataError;
use seata_sqlparser::{ColumnList, ColumnMeta, IndexMeta, IndexType, JavaObject, JavaString};
use std::{
    any::Any,
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn index_defaults_null_shortcuts_and_cardinality_contract() -> TestResult {
    let left = IndexMeta::default();
    let right = IndexMeta::default();
    assert_eq!(left.values()?.ok_or("Missing default list")?.len()?, 0);
    assert!(left.indextype()?.is_none());
    assert_eq!(left.java_hash_code()?, 1237);
    assert!(left.java_equals(Some(&left.clone()))?);
    assert_eq!(
        left.java_equals(Some(&right))
            .err()
            .ok_or("Missing null-type failure")?
            .java_exception_class(),
        Some("java.lang.NullPointerException")
    );
    right.set_values(None)?;
    assert!(!left.java_equals(Some(&right))?);
    left.set_values(None)?;
    left.set_indextype(Some(IndexType::PRIMARY))?;
    right.set_indextype(Some(IndexType::PRIMARY))?;
    left.set_cardinality(i64::MIN)?;
    right.set_cardinality(i64::MAX)?;
    assert!(left.java_equals(Some(&right))?);
    assert_eq!(left.java_hash_code()?, right.java_hash_code()?);
    assert_eq!(left.java_hash_code_with_enum_identity(|_| 9001)?, 10238);
    left.set_type(i16::MIN)?;
    assert_eq!(left.r#type()?, i16::MIN);
    left.set_type(i16::MAX)?;
    assert_eq!(left.r#type()?, i16::MAX);
    Ok(())
}

#[test]
fn list_nulls_aliases_fixed_mutability_and_index_errors() -> TestResult {
    let column = ColumnMeta::default();
    let list = ColumnList::fixed(vec![Some(column.clone()), None]);
    let alias = list.clone();
    assert!(list.same_reference(&alias));
    let index = IndexMeta::default();
    index.set_values(Some(list.clone()))?;
    column.set_column_name(Some("new name".into()))?;
    assert_eq!(
        index
            .values()?
            .ok_or("Missing values")?
            .get(0)?
            .ok_or("Missing column")?
            .column_name()?
            .as_ref()
            .and_then(JavaString::as_str),
        Some("new name")
    );
    assert!(list.get(1)?.is_none());
    alias.set(0, None)?;
    assert!(list.get(0)?.is_none());
    assert_eq!(
        list.push(None)
            .err()
            .ok_or("Fixed push accepted")?
            .java_exception_class(),
        Some("java.lang.UnsupportedOperationException")
    );
    assert_eq!(
        list.get(-1)
            .err()
            .ok_or("Negative index accepted")?
            .java_exception_class(),
        Some("java.lang.ArrayIndexOutOfBoundsException")
    );
    let empty = ColumnList::fixed(vec![]);
    empty.clear()?;
    let growable = ColumnList::default();
    growable.push(None)?;
    assert_eq!(growable.len()?, 1);
    growable.remove(0)?;
    growable.clear()?;
    assert_eq!(
        growable
            .get(0)
            .err()
            .ok_or("Invalid index accepted")?
            .java_exception_class(),
        Some("java.lang.IndexOutOfBoundsException")
    );
    Ok(())
}

#[derive(Debug)]
enum Action {
    Clear(ColumnList),
    Set(ColumnList, ColumnMeta),
    RemoveNext(ColumnList),
    Index(IndexMeta),
}
#[derive(Debug)]
struct Callback {
    action: Action,
    once: AtomicBool,
}
impl Callback {
    fn fire(&self) {
        if !self.once.swap(true, Ordering::SeqCst) {
            match &self.action {
                Action::Clear(list) => {
                    assert!(list.clear().is_ok());
                }
                Action::Set(list, column) => {
                    assert!(list.set(1, Some(column.clone())).is_ok());
                }
                Action::RemoveNext(list) => {
                    assert!(list.remove(1).is_ok());
                }
                Action::Index(index) => {
                    assert!(index.set_non_unique(true).is_ok());
                }
            }
        }
    }
}
impl JavaObject for Callback {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, _other: &dyn JavaObject) -> Result<bool, MetadataError> {
        self.fire();
        Ok(true)
    }
    fn java_hash_code(&self) -> Result<i32, MetadataError> {
        Ok(1)
    }
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        self.fire();
        Ok(Some("callback".into()))
    }
}

fn callback_column(action: Action) -> Result<ColumnMeta, Box<dyn Error>> {
    let column = ColumnMeta::default();
    column.set_char_octet_length(Some(Arc::new(Callback {
        action,
        once: AtomicBool::new(false),
    })))?;
    Ok(column)
}

#[test]
fn array_list_equality_detects_structural_change_after_callback() -> TestResult {
    let left = ColumnList::from_values(vec![
        Some(ColumnMeta::default()),
        Some(ColumnMeta::default()),
    ]);
    let right = ColumnList::from_values(vec![
        Some(callback_column(Action::Clear(left.clone()))?),
        Some(ColumnMeta::default()),
    ]);
    // ColumnMeta比较调用other字段的equals，所以回调放在right的列中。
    left.get(0)?
        .ok_or("Missing column")?
        .set_char_octet_length(Some(Arc::new(1_i32)))?;
    let error = left
        .java_equals(&right)
        .err()
        .ok_or("Missing concurrent modification")?;
    assert_eq!(
        error.java_exception_class(),
        Some("java.util.ConcurrentModificationException")
    );
    right.clear()?;
    Ok(())
}

#[test]
fn array_list_equality_observes_nonstructural_set_of_future_slot() -> TestResult {
    let left = ColumnList::from_values(vec![
        Some(ColumnMeta::default()),
        Some(ColumnMeta::default()),
    ]);
    let changed = ColumnMeta::default();
    changed.set_column_name(Some("changed".into()))?;
    let right = ColumnList::from_values(vec![
        Some(callback_column(Action::Set(left.clone(), changed))?),
        Some(ColumnMeta::default()),
    ]);
    left.get(0)?
        .ok_or("Missing column")?
        .set_char_octet_length(Some(Arc::new(1_i32)))?;
    assert!(!left.java_equals(&right)?);
    left.clear()?;
    right.clear()?;
    Ok(())
}

#[test]
fn to_string_uses_live_iterator_has_next_after_callback() -> TestResult {
    let list = ColumnList::default();
    let first = callback_column(Action::RemoveNext(list.clone()))?;
    list.push(Some(first.clone()))?;
    list.push(Some(ColumnMeta::default()))?;
    let actual = list.java_to_string()?;
    assert_eq!(list.len()?, 1);
    let mut expected = vec![91_u16];
    expected.extend(first.java_to_string()?.as_utf16());
    expected.push(93);
    assert_eq!(actual.as_utf16(), expected);
    list.clear()?;
    Ok(())
}

#[test]
fn index_reads_scalar_fields_after_list_comparison_callback() -> TestResult {
    let left = IndexMeta::default();
    let right = IndexMeta::default();
    let a = callback_column(Action::Index(left.clone()))?;
    let b = ColumnMeta::default();
    b.set_char_octet_length(Some(Arc::new(1_i32)))?;
    left.set_values(Some(ColumnList::fixed(vec![Some(a)])))?;
    right.set_values(Some(ColumnList::fixed(vec![Some(b)])))?;
    left.set_indextype(Some(IndexType::PRIMARY))?;
    right.set_indextype(Some(IndexType::PRIMARY))?;
    right.set_non_unique(true)?;
    // IndexMeta先调用other.values.equals(self.values)，对应ColumnMeta内部再反向调用对象equals。
    let actual = left.java_equals(Some(&right))?;
    assert!(actual);
    assert!(left.is_non_unique()?);
    left.set_values(None)?;
    right.set_values(None)?;
    Ok(())
}
