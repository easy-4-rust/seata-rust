//! 对应 Java: org.apache.seata.rm.datasource.sql.struct.IndexMetaTest；保留全部原断言与顺序。
#![allow(clippy::unwrap_used)]
#[path = "../../support/rm_metadata_source.rs"]
#[allow(dead_code)]
mod support;
use seata_sqlparser::{ColumnList, ColumnMeta, IndexMeta, IndexType};
use support::{check, observe, TestResult};

#[test]
fn original_rm_test_index_meta() -> TestResult {
    let index = IndexMeta::default();
    let mut values = Vec::new();
    index.set_values(Some(ColumnList::default()))?;
    check(
        &mut values,
        "not_null",
        index.java_to_string().is_ok(),
        true,
    );
    let other = IndexMeta::default();
    other.set_values(Some(ColumnList::from_values(vec![Some(
        ColumnMeta::default(),
    )])))?;
    check(
        &mut values,
        "not_equals",
        !index.java_equals(Some(&other))?,
        true,
    );
    macro_rules! other {
        ($setter:ident,$value:expr) => {
            let other = IndexMeta::default();
            other.$setter($value)?;
            check(
                &mut values,
                "not_equals",
                !index.java_equals(Some(&other))?,
                true,
            );
        };
    }
    other!(set_non_unique, true);
    other!(set_index_qualifier, Some("".into()));
    other!(set_index_name, Some("".into()));
    other!(set_type, 1);
    let other = IndexMeta::default();
    index.set_indextype(Some(IndexType::PRIMARY))?;
    other.set_indextype(Some(IndexType::NORMAL))?;
    check(
        &mut values,
        "not_equals",
        !index.java_equals(Some(&other))?,
        true,
    );
    let other = IndexMeta::default();
    other.set_asc_or_desc(Some("".into()))?;
    other.set_indextype(Some(IndexType::NORMAL))?;
    index.set_indextype(Some(IndexType::NORMAL))?;
    check(
        &mut values,
        "not_equals",
        !index.java_equals(Some(&other))?,
        true,
    );
    let other = IndexMeta::default();
    other.set_ordinal_position(1)?;
    other.set_indextype(Some(IndexType::NORMAL))?;
    index.set_indextype(Some(IndexType::NORMAL))?;
    check(
        &mut values,
        "not_equals",
        !index.java_equals(Some(&other))?,
        true,
    );
    let other = IndexMeta::default();
    other.set_indextype(Some(IndexType::NORMAL))?;
    index.set_indextype(Some(IndexType::NORMAL))?;
    check(
        &mut values,
        "equals:object",
        index.java_equals(Some(&other))?,
        true,
    );
    observe("IndexMetaTest", "testIndexMeta", &values)
}
