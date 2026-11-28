//! 元数据迁移引入的共享、重入、锁释放及集合迭代义务；不计为额外源JUnit案例。
use seata_sqlparser::{
    ColumnList, ColumnMeta, IndexMeta, IndexType, JavaObject, JavaString, LowerCaseLinkHashMap,
    MetadataError, MetadataLocale, MetadataMapValue, TableMeta,
};
use std::{
    any::Any,
    error::Error,
    fmt,
    sync::{mpsc, Arc},
    time::Duration,
};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn contains_pk_requires_every_key_and_preserves_nullable_collection_elements() -> TestResult {
    let table = TableMeta::new(MetadataLocale::new("en"));
    let first = ColumnMeta::default();
    first.set_column_name(Some("first".into()))?;
    let second = ColumnMeta::default();
    second.set_column_name(Some("second".into()))?;
    let index = Arc::new(IndexMeta::default());
    index.set_indextype(Some(IndexType::PRIMARY))?;
    index.set_values(Some(ColumnList::from_values(vec![
        Some(first),
        Some(second),
    ])))?;
    table
        .all_indexes()
        .put(Some("primary".into()), Some(index))?;
    assert!(!table.contains_pk(Some(&[Some("first".into())]))?);
    assert!(!table.contains_pk(Some(&[None, Some("FIRST".into())]))?);
    assert!(table.contains_pk(Some(&[None, Some("SECOND".into()), Some("FIRST".into())]))?);
    Ok(())
}

fn column(value: i32) -> Result<Arc<ColumnMeta>, MetadataError> {
    let col = Arc::new(ColumnMeta::default());
    col.set_data_type(value)?;
    Ok(col)
}

#[test]
fn shared_locale_changes_lookup_and_alias_map_mutations_without_rekeying_old_entries() -> TestResult
{
    let locale = MetadataLocale::new("en");
    let table = TableMeta::new(locale.clone());
    let map = table.all_columns();
    map.put(Some("I".into()), Some(column(1)?))?;
    assert_eq!(
        map.get(Some(&"I".into()))?
            .ok_or("Missing I")?
            .data_type()?,
        1
    );
    assert_eq!(locale.set_language("tr")?, "en");
    assert!(map.get(Some(&"I".into()))?.is_none());
    assert_eq!(
        map.get(Some(&"i".into()))?
            .ok_or("Missing stored i")?
            .data_type()?,
        1
    );
    map.put(Some("I".into()), Some(column(2)?))?;
    let mut cursor = map.entries()?;
    let mut keys = Vec::new();
    while let Some((key, _)) = cursor.next_entry()? {
        keys.push(key);
    }
    assert_eq!(keys, vec![JavaString::from("i"), JavaString::from("ı")]);
    map.remove(Some(&"I".into()))?;
    assert_eq!(map.len()?, 1);
    locale.set_language("en")?;
    assert_eq!(
        map.get(Some(&"I".into()))?
            .ok_or("Old i lost")?
            .data_type()?,
        1
    );
    map.clone().clear()?;
    assert!(table.all_columns().is_empty()?);
    Ok(())
}

#[test]
fn nullable_values_and_cursor_next_keep_original_structure_version_points() -> TestResult {
    let map = LowerCaseLinkHashMap::<ColumnMeta>::new(MetadataLocale::new("en"));
    assert!(map.get(None)?.is_none());
    assert!(!map.contains_key(None)?);
    assert_eq!(
        map.put(None, None)
            .err()
            .ok_or("Null key accepted")?
            .java_exception_class(),
        Some("java.lang.NullPointerException")
    );
    map.put(Some("nullValue".into()), None)?;
    assert!(map.contains_key(Some(&"NULLVALUE".into()))?);
    assert!(map.get(Some(&"nullvalue".into()))?.is_none());
    map.clear()?;
    map.put(Some("a".into()), Some(column(1)?))?;
    map.put(Some("b".into()), Some(column(2)?))?;
    let mut cursor = map.entries()?;
    assert_eq!(
        cursor.next_entry()?.ok_or("Missing a")?.0,
        JavaString::from("a")
    );
    map.put(Some("b".into()), Some(column(9)?))?;
    assert_eq!(
        cursor
            .next_entry()?
            .ok_or("Missing b")?
            .1
            .ok_or("Null b")?
            .data_type()?,
        9
    );
    map.put(Some("c".into()), None)?;
    assert!(!cursor.has_next());
    assert!(cursor.next_entry()?.is_none());
    let mut cursor = map.entries()?;
    cursor.next_entry()?;
    map.clear()?;
    assert_eq!(
        cursor
            .next_entry()
            .err()
            .ok_or("Missing CME")?
            .java_exception_class(),
        Some("java.util.ConcurrentModificationException")
    );
    Ok(())
}

struct CallbackValue {
    action: Arc<dyn Fn() -> Result<(), MetadataError> + Send + Sync>,
}
impl fmt::Debug for CallbackValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CallbackValue")
    }
}
impl JavaObject for CallbackValue {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, _other: &dyn JavaObject) -> Result<bool, MetadataError> {
        (self.action)()?;
        Ok(true)
    }
    fn java_hash_code(&self) -> Result<i32, MetadataError> {
        (self.action)()?;
        Ok(7)
    }
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(Some("callback".into()))
    }
}

#[test]
fn table_hash_reads_case_flag_after_column_callback_without_holding_table_or_map_lock() -> TestResult
{
    let table = TableMeta::new(MetadataLocale::new("en"));
    let changing = table.clone();
    let value = Arc::new(ColumnMeta::default());
    value.set_char_octet_length(Some(Arc::new(CallbackValue {
        action: Arc::new(move || changing.set_case_sensitive(true)),
    })))?;
    table.all_columns().put(Some("c".into()), Some(value))?;
    assert!(!table.is_case_sensitive()?);
    let reference = ColumnMeta::default();
    reference.set_char_octet_length(Some(Arc::new(7_i32)))?;
    let expected = (JavaString::from("c").java_hash_code() ^ reference.java_hash_code()?)
        .wrapping_add(31)
        .wrapping_add(31)
        .wrapping_add(1231);
    let (tx, rx) = mpsc::channel();
    let run = table.clone();
    let worker = std::thread::spawn(move || {
        let _ = tx.send(run.java_hash_code());
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(2))??, expected);
    worker.join().map_err(|_| "Hash worker panicked")?;
    assert!(table.is_case_sensitive()?);
    table.all_columns().clear()?;
    Ok(())
}

#[test]
fn map_equality_sees_future_replacement_and_keeps_last_callback_structure_shortcut() -> TestResult {
    let mine = TableMeta::new(MetadataLocale::new("en"));
    let theirs = TableMeta::new(MetadataLocale::new("en"));
    let future = theirs.clone();
    let callback = Arc::new(CallbackValue {
        action: Arc::new(move || {
            future
                .all_columns()
                .put(Some("b".into()), Some(column(9)?))?;
            Ok(())
        }),
    });
    let left = Arc::new(ColumnMeta::default());
    left.set_char_octet_length(Some(callback))?;
    let right = Arc::new(ColumnMeta::default());
    right.set_char_octet_length(Some(Arc::new(1_i32)))?;
    mine.all_columns().put(Some("a".into()), Some(left))?;
    theirs.all_columns().put(Some("a".into()), Some(right))?;
    mine.all_columns().put(Some("b".into()), Some(column(9)?))?;
    theirs
        .all_columns()
        .put(Some("b".into()), Some(column(2)?))?;
    assert!(mine.java_equals(Some(&theirs))?);
    mine.all_columns().clear()?;
    theirs.all_columns().clear()?;
    let clear_target = theirs.clone();
    let left = Arc::new(ColumnMeta::default());
    left.set_char_octet_length(Some(Arc::new(CallbackValue {
        action: Arc::new(move || clear_target.all_columns().clear()),
    })))?;
    let right = Arc::new(ColumnMeta::default());
    right.set_char_octet_length(Some(Arc::new(1_i32)))?;
    mine.all_columns().put(Some("a".into()), Some(left))?;
    theirs.all_columns().put(Some("a".into()), Some(right))?;
    assert!(mine.java_equals(Some(&theirs))?);
    assert!(theirs.all_columns().is_empty()?);
    mine.all_columns().clear()?;
    Ok(())
}

struct DroppingValue {
    on_drop: Arc<dyn Fn() + Send + Sync>,
}
impl Drop for DroppingValue {
    fn drop(&mut self) {
        (self.on_drop)();
    }
}
impl MetadataMapValue for DroppingValue {
    fn metadata_equals(&self, _other: Option<&Self>) -> Result<bool, MetadataError> {
        Ok(false)
    }
    fn metadata_hash(
        &self,
        _context: &mut dyn FnMut(IndexType) -> i32,
    ) -> Result<i32, MetadataError> {
        Ok(0)
    }
}

#[test]
fn clearing_map_releases_user_values_after_unlocking_even_when_drop_reads_map() -> TestResult {
    let map = LowerCaseLinkHashMap::<DroppingValue>::new(MetadataLocale::new("en"));
    let callback_map = map.clone();
    let (tx, rx) = mpsc::channel();
    map.put(
        Some("c".into()),
        Some(Arc::new(DroppingValue {
            on_drop: Arc::new(move || {
                let _ = tx.send(callback_map.is_empty());
            }),
        })),
    )?;
    let worker = std::thread::spawn(move || map.clear());
    assert!(rx.recv_timeout(Duration::from_secs(2))??);
    worker.join().map_err(|_| "Clear worker panicked")??;
    Ok(())
}
