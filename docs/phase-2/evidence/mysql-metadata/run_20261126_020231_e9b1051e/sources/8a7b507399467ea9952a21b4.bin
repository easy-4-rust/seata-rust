//! 原IndexMetaTest的全部16案例；末两案例按原代码验证ColumnMeta，不冒充IndexMeta哈希覆盖。
#[path = "support/observations.rs"]
mod observations;
use seata_sqlparser::{ColumnList, ColumnMeta, IndexMeta, IndexType};
use std::{error::Error, sync::Arc};
type TestResult = Result<(), Box<dyn Error>>;

fn record(method: &str, values: &[String]) -> TestResult {
    observations::record("struct.IndexMetaTest", method, values)
}

#[test]
fn get_set_values() -> TestResult {
    let index = IndexMeta::default();
    let values = ColumnList::fixed(vec![
        Some(ColumnMeta::default()),
        Some(ColumnMeta::default()),
    ]);
    index.set_values(Some(values.clone()))?;
    let actual = index.values()?.ok_or("Missing index values")?;
    assert!(values.java_equals(&actual)?);
    assert!(values.same_reference(&actual));
    record(
        "testGetSetValues",
        &[values.java_equals(&actual)?.to_string()],
    )
}

#[test]
fn is_set_non_unique() -> TestResult {
    let index = IndexMeta::default();
    index.set_non_unique(true)?;
    let yes = index.is_non_unique()?;
    assert!(yes);
    index.set_non_unique(false)?;
    let no = index.is_non_unique()?;
    assert!(!no);
    record("testIsSetNonUnique", &[yes.to_string(), no.to_string()])
}

macro_rules! string_case {
    ($name:ident, $source:literal, $setter:ident, $getter:ident, $value:literal) => {
        #[test]
        fn $name() -> TestResult {
            let index = IndexMeta::default();
            index.$setter(Some($value.into()))?;
            let actual = index.$getter()?.ok_or("Missing index string")?;
            assert_eq!(actual.as_str(), Some($value));
            record($source, &[actual.to_string()])
        }
    };
}
string_case!(
    get_set_index_qualifier,
    "testGetSetIndexQualifier",
    set_index_qualifier,
    index_qualifier,
    "qualifier"
);
string_case!(
    get_set_index_name,
    "testGetSetIndexName",
    set_index_name,
    index_name,
    "indexName"
);
string_case!(
    get_set_asc_or_desc,
    "testGetSetAscOrDesc",
    set_asc_or_desc,
    asc_or_desc,
    "A"
);

macro_rules! number_case {
    ($name:ident, $source:literal, $setter:ident, $getter:ident, $value:literal) => {
        #[test]
        fn $name() -> TestResult {
            let index = IndexMeta::default();
            index.$setter($value)?;
            let actual = index.$getter()?;
            assert_eq!(actual, $value);
            record($source, &[actual.to_string()])
        }
    };
}
number_case!(get_set_type, "testGetSetType", set_type, r#type, 1);
number_case!(
    get_set_cardinality,
    "testGetSetCardinality",
    set_cardinality,
    cardinality,
    100
);
number_case!(
    get_set_ordinal_position,
    "testGetSetOrdinalPosition",
    set_ordinal_position,
    ordinal_position,
    1
);

#[test]
fn get_set_indextype() -> TestResult {
    let index = IndexMeta::default();
    index.set_indextype(Some(IndexType::NORMAL))?;
    let actual = index.indextype()?.ok_or("Missing indextype")?;
    assert_eq!(actual, IndexType::NORMAL);
    record("testGetSetIndextype", &[actual.to_string()])
}

#[test]
fn equals_same_object() -> TestResult {
    let index = IndexMeta::default();
    let actual = index.java_equals(Some(&index))?;
    assert!(actual);
    record("testEqualsSameObject", &[actual.to_string()])
}

#[test]
fn equals_null_object() -> TestResult {
    let index = IndexMeta::default();
    let actual = index.java_equals(None)?;
    assert!(!actual);
    record("testEqualsNullObject", &[actual.to_string()])
}

#[test]
fn equals_different_type() -> TestResult {
    let index = IndexMeta::default();
    let different_type = "I am not an IndexMeta";
    let actual = index.java_equals(Some(&different_type))?;
    assert!(!actual);
    record("testEqualsDifferentType", &[actual.to_string()])
}

fn index_fixture(second: bool, different: bool) -> Result<IndexMeta, Box<dyn Error>> {
    let index = IndexMeta::default();
    index.set_values(Some(ColumnList::fixed(
        (0..if different { 3 } else { 2 })
            .map(|_| Some(ColumnMeta::default()))
            .collect(),
    )))?;
    index.set_non_unique(!(second && different))?;
    index.set_index_qualifier(Some(
        if second && different {
            "differentQualifier"
        } else {
            "qualifier"
        }
        .into(),
    ))?;
    index.set_index_name(Some(
        if second && different {
            "differentIndexName"
        } else {
            "indexName"
        }
        .into(),
    ))?;
    index.set_type(if second && different { 2 } else { 1 })?;
    index.set_indextype(Some(if different {
        if second {
            IndexType::FULL_TEXT
        } else {
            IndexType::UNIQUE
        }
    } else {
        IndexType::PRIMARY
    }))?;
    index.set_asc_or_desc(Some(if second && different { "D" } else { "A" }.into()))?;
    index.set_ordinal_position(if second && different { 2 } else { 1 })?;
    Ok(index)
}

#[test]
fn equals_identical_objects() -> TestResult {
    let left = index_fixture(false, false)?;
    let right = index_fixture(true, false)?;
    let actual = left.java_equals(Some(&right))?;
    assert!(actual);
    record("testEqualsIdenticalObjects", &[actual.to_string()])
}

#[test]
fn equals_different_objects() -> TestResult {
    let left = index_fixture(false, true)?;
    let right = index_fixture(true, true)?;
    let actual = left.java_equals(Some(&right))?;
    assert!(!actual);
    record("testEqualsDifferentObjects", &[actual.to_string()])
}

fn column_fixture(variant: usize) -> Result<ColumnMeta, Box<dyn Error>> {
    let column = ColumnMeta::default();
    let suffix = match variant {
        0 => "",
        1 => "1",
        _ => "2",
    };
    column.set_table_cat(Some(format!("tableCat{suffix}").into()))?;
    column.set_table_schema_name(Some(format!("schemaName{suffix}").into()))?;
    column.set_table_name(Some(format!("tableName{suffix}").into()))?;
    column.set_column_name(Some(format!("columnName{suffix}").into()))?;
    column.set_data_type(if variant == 2 { 2 } else { 1 })?;
    column.set_data_type_name(Some(format!("dataTypeName{suffix}").into()))?;
    column.set_column_size(if variant == 2 { 200 } else { 100 })?;
    column.set_decimal_digits(if variant == 2 { 20 } else { 10 })?;
    column.set_num_prec_radix(if variant == 2 { 20 } else { 10 })?;
    column.set_null_able(if variant == 2 { 0 } else { 1 })?;
    column.set_remarks(Some(format!("remarks{suffix}").into()))?;
    column.set_column_def(Some(format!("columnDef{suffix}").into()))?;
    column.set_sql_data_type(if variant == 2 { 2 } else { 1 })?;
    column.set_sql_datetime_sub(if variant == 2 { 2 } else { 1 })?;
    column.set_char_octet_length(Some(Arc::new(if variant == 2 { 200_i32 } else { 100_i32 })))?;
    column.set_ordinal_position(if variant == 2 { 2 } else { 1 })?;
    column.set_is_null_able(Some(if variant == 2 { "NO" } else { "YES" }.into()))?;
    column.set_is_autoincrement(Some(if variant == 2 { "NO" } else { "YES" }.into()))?;
    column.set_on_update(variant != 2)?;
    column.set_case_sensitive(variant != 2)?;
    Ok(column)
}

#[test]
fn hash_code_consistency_original_column_assertions() -> TestResult {
    let left = column_fixture(0)?;
    let right = column_fixture(0)?;
    let actual = left.java_hash_code()? == right.java_hash_code()?;
    assert!(actual);
    record("testHashCodeConsistency", &[actual.to_string()])
}

#[test]
fn hash_code_difference_original_column_assertions() -> TestResult {
    let left = column_fixture(1)?;
    let right = column_fixture(2)?;
    let actual = left.java_hash_code()? != right.java_hash_code()?;
    assert!(actual);
    record("testHashCodeDifference", &[actual.to_string()])
}
