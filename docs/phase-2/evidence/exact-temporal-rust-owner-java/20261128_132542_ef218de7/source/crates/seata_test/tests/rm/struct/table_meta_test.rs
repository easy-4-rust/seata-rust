//! 对应 Java: org.apache.seata.rm.datasource.sql.struct.TableMetaTest；保留全部原断言与顺序。
#![allow(clippy::unwrap_used)]
#[path = "../../support/rm_metadata_source.rs"]
#[allow(dead_code)]
mod support;
use seata_sqlparser::{ColumnMeta, IndexMeta, JavaString};
use std::sync::Arc;
use support::{check, observe, primary, table, TestResult};

#[test]
fn original_rm_test_table_meta() -> TestResult {
    let current = table();
    let mut values = Vec::new();
    check(
        &mut values,
        "equals:object",
        current.java_equals(Some(&table()))?,
        true,
    );
    let other = table();
    other.set_table_name(Some("".into()))?;
    check(
        &mut values,
        "not_equals",
        !current.java_equals(Some(&other))?,
        true,
    );
    let other = table();
    other.all_columns().put(
        Some("columnName".into()),
        Some(Arc::new(ColumnMeta::default())),
    )?;
    check(
        &mut values,
        "not_equals",
        !current.java_equals(Some(&other))?,
        true,
    );
    let other = table();
    other.all_indexes().put(
        Some("indexName".into()),
        Some(Arc::new(IndexMeta::default())),
    )?;
    check(
        &mut values,
        "not_equals",
        !current.java_equals(Some(&other))?,
        true,
    );
    observe("TableMetaTest", "testTableMeta", &values)
}

#[test]
fn original_rm_test_get_column_meta() -> TestResult {
    let t = table();
    let mut values = Vec::new();
    for name in ["id", "name"] {
        t.all_columns()
            .put(Some(name.into()), Some(Arc::new(ColumnMeta::default())))?;
    }
    check(
        &mut values,
        "null",
        t.column_meta(Some(&JavaString::from_utf16(vec![96, 105, 100, 96])))?
            .is_none(),
        true,
    );
    check(
        &mut values,
        "not_null",
        t.column_meta(Some(&"name".into()))?.is_some(),
        true,
    );
    observe("TableMetaTest", "testGetColumnMeta", &values)
}

#[test]
fn original_rm_test_get_auto_increase_column() -> TestResult {
    let t = table();
    let mut values = Vec::new();
    let id = ColumnMeta::default();
    id.set_is_autoincrement(Some("YES".into()))?;
    t.all_columns().put(Some("id".into()), Some(Arc::new(id)))?;
    check(
        &mut values,
        "not_null",
        t.auto_increase_column()?.is_some(),
        true,
    );
    let t = table();
    t.all_columns()
        .put(Some("name".into()), Some(Arc::new(ColumnMeta::default())))?;
    check(
        &mut values,
        "null",
        t.auto_increase_column()?.is_none(),
        true,
    );
    observe("TableMetaTest", "testGetAutoIncreaseColumn", &values)
}

#[test]
fn original_rm_test_get_primary_key_map() -> TestResult {
    let t = table();
    let mut values = Vec::new();
    t.all_indexes()
        .put(Some("id".into()), Some(Arc::new(primary(None)?)))?;
    check(&mut values, "not_null", t.primary_key_map().is_ok(), true);
    observe("TableMetaTest", "testGetPrimaryKeyMap", &values)
}

#[test]
fn original_rm_test_get_primary_key_only_name() -> TestResult {
    let t = table();
    let mut values = Vec::new();
    for name in ["id", "userCode"] {
        t.all_indexes()
            .put(Some(name.into()), Some(Arc::new(primary(Some(name))?)))?;
    }
    let names = t.primary_key_only_name()?;
    assert_eq!(names.len(), 2);
    values.push(format!("equals:{}", names.len()));
    check(
        &mut values,
        "true",
        names.contains(&Some("id".into())),
        true,
    );
    check(
        &mut values,
        "true",
        names.contains(&Some("userCode".into())),
        true,
    );
    observe("TableMetaTest", "testGetPrimaryKeyOnlyName", &values)
}

#[test]
fn original_rm_test_get_pk_name() -> TestResult {
    let t = table();
    t.all_indexes()
        .put(Some("id".into()), Some(Arc::new(primary(Some("id"))?)))?;
    let names = t.primary_key_only_name()?;
    assert_eq!(names.first(), Some(&Some(JavaString::from("id"))));
    observe(
        "TableMetaTest",
        "testGetPkName",
        &[format!(
            "equals:{}",
            names[0].as_ref().ok_or("missing original key")?
        )],
    )
}

#[test]
fn original_rm_test_contains_pk() -> TestResult {
    let t = table();
    let mut values = Vec::new();
    check(&mut values, "false", t.contains_pk(None)?, false);
    let error = t.contains_pk(Some(&[Some("id".into())])).unwrap_err();
    assert_eq!(
        error.java_exception_class(),
        Some("org.apache.seata.common.exception.NotSupportYetException")
    );
    values.push(format!(
        "throws:{}",
        error
            .java_exception_class()
            .ok_or("missing exception type")?
    ));
    let message = error
        .java_message()
        .ok_or("missing original exception message")?;
    assert_eq!(
        message.as_str(),
        Some("null needs to contain the primary key.")
    );
    values.push(format!("equals:{message}"));
    t.all_indexes()
        .put(Some("id".into()), Some(Arc::new(primary(Some("id"))?)))?;
    check(
        &mut values,
        "true",
        t.contains_pk(Some(&[Some("id".into())]))?,
        true,
    );
    check(
        &mut values,
        "true",
        t.contains_pk(Some(&[Some("ID".into())]))?,
        true,
    );
    observe("TableMetaTest", "testContainsPK", &values)
}
