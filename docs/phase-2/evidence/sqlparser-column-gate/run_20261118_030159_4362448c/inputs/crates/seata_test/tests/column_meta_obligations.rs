//! 共享身份、动态对象和原Java测试未覆盖的真实语义分支。
use seata_sqlparser::{ColumnMeta, JavaObject, JavaString};
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
fn autoincrement_uses_java_unicode_ignore_case_not_ascii_only() -> TestResult {
    let column = ColumnMeta::default();
    column.set_is_autoincrement(Some("yeſ".into()))?;
    assert!(
        column.is_autoincrement()?,
        "Java YES.equalsIgnoreCase accepts long s U+017F"
    );
    column.set_is_autoincrement(Some("YES ".into()))?;
    assert!(!column.is_autoincrement()?);
    column.set_is_autoincrement(None)?;
    assert!(!column.is_autoincrement()?);
    Ok(())
}

#[test]
fn clone_shares_column_mutations_and_allows_clearing() -> TestResult {
    let column = ColumnMeta::default();
    let alias = column.clone();
    alias.set_column_name(Some(JavaString::from_utf16(vec![0xd800, 0, 0xdc00])))?;
    assert_eq!(
        column.column_name()?.ok_or("Missing raw name")?.as_utf16(),
        [0xd800, 0, 0xdc00]
    );
    column.set_column_name(None)?;
    assert!(alias.column_name()?.is_none());
    assert!(column.java_equals(Some(&alias))?);
    assert!(!column.java_equals(None)?);
    assert!(!column.java_equals(Some(&"not a ColumnMeta"))?);
    Ok(())
}

#[test]
fn object_field_preserves_boxed_types_and_null() -> TestResult {
    let left = ColumnMeta::default();
    let right = ColumnMeta::default();
    left.set_char_octet_length(Some(Arc::new(255_i32)))?;
    right.set_char_octet_length(Some(Arc::new(255_i64)))?;
    assert!(!left.java_equals(Some(&right))?);
    right.set_char_octet_length(Some(Arc::new(255_i32)))?;
    assert!(left.java_equals(Some(&right))?);
    assert_eq!(left.java_hash_code()?, right.java_hash_code()?);
    right.set_char_octet_length(None)?;
    assert!(!left.java_equals(Some(&right))?);
    assert!(right.char_octet_length()?.is_none());
    Ok(())
}

#[derive(Debug)]
struct ReentrantDrop {
    column: ColumnMeta,
    ran: Arc<AtomicBool>,
}
impl Drop for ReentrantDrop {
    fn drop(&mut self) {
        self.ran.store(
            self.column
                .set_remarks(Some("released outside lock".into()))
                .is_ok(),
            Ordering::SeqCst,
        );
    }
}
impl JavaObject for ReentrantDrop {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, other: &dyn JavaObject) -> bool {
        std::ptr::eq(self as &dyn JavaObject, other)
    }
    fn java_hash_code(&self) -> i32 {
        71
    }
    fn java_to_string(&self) -> Option<JavaString> {
        None
    }
}

#[test]
fn replacing_dynamic_object_releases_user_drop_outside_metadata_lock() -> TestResult {
    let column = ColumnMeta::default();
    let ran = Arc::new(AtomicBool::new(false));
    column.set_char_octet_length(Some(Arc::new(ReentrantDrop {
        column: column.clone(),
        ran: Arc::clone(&ran),
    })))?;
    assert!(column
        .java_to_string()?
        .as_str()
        .ok_or("Not Unicode")?
        .contains("charOctetLength=null"));
    column.set_char_octet_length(None)?;
    assert!(ran.load(Ordering::SeqCst));
    assert_eq!(
        column.remarks()?.as_ref().and_then(JavaString::as_str),
        Some("released outside lock")
    );
    Ok(())
}

#[test]
fn java_hash_wraps_and_text_retains_raw_utf16() -> TestResult {
    let column = ColumnMeta::default();
    assert_eq!(column.java_hash_code()?, 2474);
    column.set_data_type(i32::MAX)?;
    column.set_column_size(i32::MAX)?;
    assert_eq!(column.java_hash_code()?, 2472);
    column.set_column_name(Some(JavaString::from_utf16(vec![0xd800])))?;
    let actual = column.java_to_string()?;
    assert!(actual.as_utf16().contains(&0xd800));
    assert!(actual.as_str().is_none());
    assert_eq!(column.java_hash_code()?, 2472 + 0xd800);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum CallbackKind {
    Equals,
    Hash,
    Text,
}

#[derive(Debug)]
struct CallbackObject {
    column: ColumnMeta,
    kind: CallbackKind,
}
impl JavaObject for CallbackObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, other: &dyn JavaObject) -> bool {
        if matches!(self.kind, CallbackKind::Equals) {
            assert!(self.column.set_ordinal_position(7).is_ok());
        }
        other.as_any().is::<Self>()
    }
    fn java_hash_code(&self) -> i32 {
        if matches!(self.kind, CallbackKind::Hash) {
            assert!(self.column.set_ordinal_position(7).is_ok());
        }
        11
    }
    fn java_to_string(&self) -> Option<JavaString> {
        if matches!(self.kind, CallbackKind::Text) {
            assert!(self.column.set_ordinal_position(7).is_ok());
            assert!(self
                .column
                .set_is_null_able(Some("callback null".into()))
                .is_ok());
            assert!(self.column.set_is_autoincrement(Some("YES".into())).is_ok());
            assert!(self.column.set_on_update(true).is_ok());
            assert!(self.column.set_case_sensitive(true).is_ok());
        }
        Some("callback".into())
    }
}

#[test]
fn equals_reads_tail_fields_after_reentrant_object_callback() -> TestResult {
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
    let actual = left.java_equals(Some(&right))?;
    assert_eq!(left.ordinal_position()?, 7);
    left.set_char_octet_length(None)?;
    right.set_char_octet_length(None)?;
    assert!(
        actual,
        "Java reads ordinalPosition after Object.equals callback"
    );
    Ok(())
}

#[test]
fn hash_reads_tail_fields_after_reentrant_object_callback() -> TestResult {
    let column = ColumnMeta::default();
    column.set_char_octet_length(Some(Arc::new(CallbackObject {
        column: column.clone(),
        kind: CallbackKind::Hash,
    })))?;
    let actual = column.java_hash_code()?;
    column.set_char_octet_length(None)?;
    assert_eq!(column.ordinal_position()?, 7);
    assert_eq!(
        actual, 2492,
        "Java reads ordinalPosition after Object.hashCode callback"
    );
    Ok(())
}

#[test]
fn text_reads_tail_fields_after_reentrant_object_callback() -> TestResult {
    let column = ColumnMeta::default();
    column.set_char_octet_length(Some(Arc::new(CallbackObject {
        column: column.clone(),
        kind: CallbackKind::Text,
    })))?;
    let actual = column.java_to_string()?;
    column.set_char_octet_length(None)?;
    let text = actual.as_str().ok_or("Missing text")?;
    assert!(text.contains("ordinalPosition=7"));
    assert!(text.contains("isNullAble='callback null'"));
    assert!(text.contains("isAutoincrement='YES'"));
    assert!(text.contains("isOnUpdate=true, isCaseSensitive=true"));
    Ok(())
}

struct ReentrantDebug(ColumnMeta);
impl std::fmt::Debug for ReentrantDebug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0
            .set_remarks(Some("debug callback".into()))
            .map_err(|_| std::fmt::Error)?;
        f.write_str("ReentrantDebug")
    }
}
impl JavaObject for ReentrantDebug {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, other: &dyn JavaObject) -> bool {
        other.as_any().is::<Self>()
    }
    fn java_hash_code(&self) -> i32 {
        1
    }
    fn java_to_string(&self) -> Option<JavaString> {
        Some("debug fixture".into())
    }
}

#[test]
fn debug_does_not_call_user_formatting_under_metadata_lock() -> TestResult {
    let column = ColumnMeta::default();
    column.set_char_octet_length(Some(Arc::new(ReentrantDebug(column.clone()))))?;
    let worker_column = column.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ = sender.send(format!("{worker_column:?}"));
    });
    let result = receiver.recv_timeout(std::time::Duration::from_secs(1));
    assert!(
        result.is_ok(),
        "Debug blocked by a user callback acquiring the column write lock"
    );
    worker.join().map_err(|_| "Debug worker panicked")?;
    column.set_char_octet_length(None)?;
    Ok(())
}
