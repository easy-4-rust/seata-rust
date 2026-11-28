//! 原TableMetaTest全部19案例；显式语言依赖绑定原JDK默认zh环境，不冒充完整默认Locale启动已完成。
#[path = "support/observations.rs"]
mod observations;
use seata_sqlparser::{ColumnList, ColumnMeta, IndexMeta, IndexType, MetadataLocale, TableMeta};
use std::{error::Error, sync::Arc};

type TestResult = Result<(), Box<dyn Error>>;

fn fixture() -> Result<(TableMeta, TableMeta), Box<dyn Error>> {
    let locale = MetadataLocale::new("zh");
    let table = TableMeta::new(locale.clone());
    let table2 = TableMeta::new(locale);
    table.set_table_name(Some("tableName".into()))?;
    table2.set_table_name(Some("tableName".into()))?;
    let col1 = Arc::new(ColumnMeta::default());
    col1.set_column_name(Some("col1".into()))?;
    col1.set_on_update(true)?;
    let col2 = Arc::new(ColumnMeta::default());
    col2.set_column_name(Some("col2".into()))?;
    let primary = Arc::new(IndexMeta::default());
    primary.set_indextype(Some(IndexType::PRIMARY))?;
    primary.set_values(Some(ColumnList::fixed(vec![
        Some(col1.as_ref().clone()),
        Some(col2.as_ref().clone()),
    ])))?;
    for current in [&table, &table2] {
        current
            .all_columns()
            .put(Some("col1".into()), Some(col1.clone()))?;
        current
            .all_columns()
            .put(Some("col2".into()), Some(col2.clone()))?;
        current
            .all_indexes()
            .put(Some("primary".into()), Some(primary.clone()))?;
    }
    Ok((table, table2))
}
fn record(method: &str, values: &[String]) -> TestResult {
    observations::record("struct.TableMetaTest", method, values)
}

#[test]
fn set_get_table_name() -> TestResult {
    let (t, _) = fixture()?;
    let name = t.table_name()?.ok_or("Missing table name")?;
    assert_eq!(name.as_str(), Some("tableName"));
    record("testSetGetTableName", &[name.to_string()])
}
#[test]
fn set_get_case_sensitive() -> TestResult {
    let (t, _) = fixture()?;
    t.set_case_sensitive(true)?;
    let a = t.is_case_sensitive()?;
    assert!(a);
    t.set_case_sensitive(false)?;
    let b = t.is_case_sensitive()?;
    assert!(!b);
    record("testSetGetCaseSensitive", &[a.to_string(), b.to_string()])
}
#[test]
fn get_column_meta() -> TestResult {
    let (t, _) = fixture()?;
    let value = Arc::new(ColumnMeta::default());
    t.all_columns()
        .put(Some("col1".into()), Some(value.clone()))?;
    let actual = t
        .column_meta(Some(&"col1".into()))?
        .ok_or("Missing column")?;
    let equal = value.java_equals(Some(actual.as_ref()))?;
    assert!(equal);
    assert!(Arc::ptr_eq(&value, &actual));
    record("testGetColumnMeta", &[equal.to_string()])
}
#[test]
fn get_all_columns() -> TestResult {
    let (t, _) = fixture()?;
    t.all_columns()
        .put(Some("col1".into()), Some(Arc::new(ColumnMeta::default())))?;
    t.all_columns()
        .put(Some("col2".into()), Some(Arc::new(ColumnMeta::default())))?;
    let map = t.all_columns();
    let n = map.len()?;
    let a = map.contains_key(Some(&"col1".into()))?;
    let b = map.contains_key(Some(&"col2".into()))?;
    assert_eq!(n, 2);
    assert!(a);
    assert!(b);
    record(
        "testGetAllColumns",
        &[n.to_string(), a.to_string(), b.to_string()],
    )
}
#[test]
fn get_all_indexes() -> TestResult {
    let (t, _) = fixture()?;
    let map = t.all_indexes();
    let n = map.len()?;
    let has = map.contains_key(Some(&"primary".into()))?;
    assert_eq!(n, 1);
    assert!(has);
    record("testGetAllIndexes", &[n.to_string(), has.to_string()])
}
#[test]
fn get_primary_key_map() -> TestResult {
    let (t, _) = fixture()?;
    let map = t.primary_key_map()?;
    let n = map.len();
    let a = map.contains_key(Some(&"col1".into()));
    let b = map.contains_key(Some(&"col2".into()));
    assert_eq!(n, 2);
    assert!(a);
    assert!(b);
    record(
        "testGetPrimaryKeyMap",
        &[n.to_string(), a.to_string(), b.to_string()],
    )
}
#[test]
fn get_primary_key_map_no_primary_key() -> TestResult {
    let (t, _) = fixture()?;
    t.all_indexes().clear()?;
    let error = t
        .primary_key_map()
        .err()
        .ok_or("Expected missing primary key")?;
    assert_eq!(
        error.java_exception_class(),
        Some("org.apache.seata.common.exception.NotSupportYetException")
    );
    assert_eq!(
        error.java_message().and_then(|s| s.as_str()),
        Some("tableName needs to contain the primary key.")
    );
    record(
        "testGetPrimaryKeyMapNoPrimaryKey",
        &[
            error.java_exception_class().ok_or("Missing class")?.into(),
            error.java_message().ok_or("Missing message")?.to_string(),
        ],
    )
}
#[test]
fn get_case_insensitive_pks() -> TestResult {
    let (t, _) = fixture()?;
    t.column_meta(Some(&"col2".into()))?
        .ok_or("Missing column")?
        .set_column_name(Some("CoL2".into()))?;
    let set = t.case_insensitive_pks()?;
    let n = set.len();
    let a = set.contains(Some(&"col1".into()))?;
    let b = set.contains(Some(&"CoL2".into()))?;
    assert_eq!(n, 2);
    assert!(a);
    assert!(b);
    record(
        "testGetCaseInsensitivePKs",
        &[n.to_string(), a.to_string(), b.to_string()],
    )
}
#[test]
fn get_case_insensitive_pks_no_primary_key() -> TestResult {
    let (t, _) = fixture()?;
    t.all_indexes().clear()?;
    let error = t
        .case_insensitive_pks()
        .err()
        .ok_or("Expected missing primary key")?;
    assert_eq!(
        error.java_exception_class(),
        Some("org.apache.seata.common.exception.NotSupportYetException")
    );
    assert_eq!(
        error.java_message().and_then(|s| s.as_str()),
        Some("tableName needs to contain the primary key.")
    );
    record(
        "testGetCaseInsensitivePKsNoPrimaryKey",
        &[
            error.java_exception_class().ok_or("Missing class")?.into(),
            error.java_message().ok_or("Missing message")?.to_string(),
        ],
    )
}
#[test]
fn get_primary_key_only_name() -> TestResult {
    let (t, _) = fixture()?;
    let names = t.primary_key_only_name()?;
    let n = names.len();
    let a = names.contains(&Some("col1".into()));
    let b = names.contains(&Some("col2".into()));
    assert_eq!(n, 2);
    assert!(a);
    assert!(b);
    record(
        "testGetPrimaryKeyOnlyName",
        &[n.to_string(), a.to_string(), b.to_string()],
    )
}
#[test]
fn get_on_update_columns_only_name() -> TestResult {
    let (t, _) = fixture()?;
    let actual = t.on_update_columns_only_name()?;
    let expected = [Some("col1".into())];
    assert_eq!(actual.len(), expected.len());
    let has = expected.iter().all(|name| actual.contains(name));
    assert!(has);
    record(
        "testGetOnUpdateColumnsOnlyName",
        &[actual.len().to_string(), has.to_string()],
    )
}
#[test]
fn get_on_update_columns_only_name_no_updates() -> TestResult {
    let (t, _) = fixture()?;
    let mut cursor = t.all_columns().entries()?;
    while let Some((_, col)) = cursor.next_entry()? {
        col.ok_or("Null column")?.set_on_update(false)?;
    }
    let empty = t.on_update_columns_only_name()?.is_empty();
    assert!(empty);
    record(
        "testGetOnUpdateColumnsOnlyNameNoUpdates",
        &[empty.to_string()],
    )
}
#[test]
fn contains_pk_with_null_list() -> TestResult {
    let (t, _) = fixture()?;
    let result = t.contains_pk(None)?;
    assert!(!result);
    record("testContainsPKWithNullList", &[result.to_string()])
}
#[test]
fn contains_pk_with_no_primary_key() -> TestResult {
    let (t, _) = fixture()?;
    let result = t.contains_pk(Some(&[Some("col3".into()), Some("col4".into())]))?;
    assert!(!result);
    record("testContainsPKWithNoPrimaryKey", &[result.to_string()])
}
#[test]
fn contains_pk_with_exact_match() -> TestResult {
    let (t, _) = fixture()?;
    let result = t.contains_pk(Some(&[Some("col1".into()), Some("col2".into())]))?;
    assert!(result);
    record("testContainsPKWithExactMatch", &[result.to_string()])
}
#[test]
fn contains_pk_with_case_insensitive_match() -> TestResult {
    let (t, _) = fixture()?;
    let result = t.contains_pk(Some(&[Some("COL1".into()), Some("COL2".into())]))?;
    assert!(result);
    record(
        "testContainsPKWithCaseInsensitiveMatch",
        &[result.to_string()],
    )
}
#[test]
fn contains_pk_with_no_match() -> TestResult {
    let (t, _) = fixture()?;
    let result = t.contains_pk(Some(&[Some("other".into())]))?;
    assert!(!result);
    record("testContainsPKWithNoMatch", &[result.to_string()])
}
#[test]
fn equals() -> TestResult {
    let (t, t2) = fixture()?;
    let a = t.java_equals(Some(&t2))?;
    assert!(a);
    t2.set_table_name(Some("different_table".into()))?;
    let b = t.java_equals(Some(&t2))?;
    assert!(!b);
    record("testEquals", &[a.to_string(), b.to_string()])
}
#[test]
fn hash_code() -> TestResult {
    let (t, t2) = fixture()?;
    let a = t.java_hash_code()?;
    let b = t2.java_hash_code()?;
    assert_eq!(a, b);
    t2.set_table_name(Some("different_table".into()))?;
    let c = t.java_hash_code()?;
    let d = t2.java_hash_code()?;
    assert_ne!(c, d);
    record(
        "testHashCode",
        &[(a == b).to_string(), (c != d).to_string()],
    )
}
