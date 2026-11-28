//! 对应原 ColumnMetaTest 的全部18案例；保留变更顺序、值及相等/哈希断言。
#[path = "support/observations.rs"]
mod observations;
use seata_sqlparser::{ColumnMeta, JavaObject, JavaString};
use std::{error::Error, sync::Arc};
type TestResult = Result<(), Box<dyn Error>>;

fn record(method: &str, values: &[String]) -> TestResult {
    observations::record("struct.ColumnMetaTest", method, values)
}

#[test]
fn equals_and_hash_code() -> TestResult {
    let column1 = ColumnMeta::default();
    let column2 = ColumnMeta::default();
    let initial_equal = column1.java_equals(Some(&column2))?;
    let initial_hash_equal = column1.java_hash_code()? == column2.java_hash_code()?;
    assert!(initial_equal);
    assert!(initial_hash_equal);
    column1.set_table_name(Some("table1".into()))?;
    column1.set_column_name(Some("column1".into()))?;
    column1.set_data_type(1)?;
    assert!(!column1.java_equals(Some(&column2))?);
    assert_ne!(column1.java_hash_code()?, column2.java_hash_code()?);
    let column3 = ColumnMeta::default();
    column3.set_table_name(Some("table1".into()))?;
    column3.set_column_name(Some("column1".into()))?;
    column3.set_data_type(1)?;
    assert!(column1.java_equals(Some(&column3))?);
    assert_eq!(column1.java_hash_code()?, column3.java_hash_code()?);
    record(
        "testEqualsAndHashCode",
        &[
            initial_equal.to_string(),
            initial_hash_equal.to_string(),
            (!column1.java_equals(Some(&column2))?).to_string(),
            (column1.java_hash_code()? != column2.java_hash_code()?).to_string(),
            column1.java_equals(Some(&column3))?.to_string(),
            (column1.java_hash_code()? == column3.java_hash_code()?).to_string(),
        ],
    )
}

#[test]
fn autoincrement() -> TestResult {
    let column = ColumnMeta::default();
    let initial = column.is_autoincrement()?;
    assert!(!initial);
    column.set_is_autoincrement(Some("YES".into()))?;
    let yes = column.is_autoincrement()?;
    assert!(yes);
    column.set_is_autoincrement(Some("NO".into()))?;
    let no = column.is_autoincrement()?;
    assert!(!no);
    record(
        "testAutoincrement",
        &[initial.to_string(), yes.to_string(), no.to_string()],
    )
}

macro_rules! string_case {
    ($test:ident, $source:literal, $setter:ident, $getter:ident, $value:literal) => {
        #[test]
        fn $test() -> TestResult {
            let column = ColumnMeta::default();
            column.$setter(Some(JavaString::from($value)))?;
            let actual = column.$getter()?.ok_or("Missing string field")?;
            assert_eq!(actual.as_str(), Some($value));
            record($source, &[actual.to_string()])
        }
    };
}

string_case!(
    get_table_cat,
    "testGetTableCat",
    set_table_cat,
    table_cat,
    "tableCat"
);
string_case!(
    set_get_column_name,
    "testSetGetColumnName",
    set_column_name,
    column_name,
    "columnName"
);
string_case!(
    set_get_data_type_name,
    "testSetGetDataTypeName",
    set_data_type_name,
    data_type_name,
    "dataTypeName"
);
string_case!(
    set_get_remarks,
    "testSetGetRemarks",
    set_remarks,
    remarks,
    "remarks"
);
string_case!(
    set_get_column_def,
    "testSetGetColumnDef",
    set_column_def,
    column_def,
    "columnDef"
);

macro_rules! number_case {
    ($test:ident, $source:literal, $setter:ident, $getter:ident, $value:literal) => {
        #[test]
        fn $test() -> TestResult {
            let column = ColumnMeta::default();
            column.$setter($value)?;
            let actual = column.$getter()?;
            assert_eq!(actual, $value);
            record($source, &[actual.to_string()])
        }
    };
}

number_case!(
    set_get_data_type,
    "testSetGetDataType",
    set_data_type,
    data_type,
    2
);
number_case!(
    set_get_column_size,
    "testSetGetColumnSize",
    set_column_size,
    column_size,
    2
);
number_case!(
    set_get_decimal_digits,
    "testSetGetDecimalDigits",
    set_decimal_digits,
    decimal_digits,
    2
);
number_case!(
    set_get_num_prec_radix,
    "testSetGetNumPrecRadix",
    set_num_prec_radix,
    num_prec_radix,
    10
);
number_case!(
    set_get_null_able,
    "testSetGetNullAble",
    set_null_able,
    null_able,
    5
);
number_case!(
    set_get_sql_data_type,
    "testSetGetSqlDataType",
    set_sql_data_type,
    sql_data_type,
    1
);
number_case!(
    set_get_sql_datetime_sub,
    "testSetGetSqlDatetimeSub",
    set_sql_datetime_sub,
    sql_datetime_sub,
    2
);
number_case!(
    set_get_ordinal_position,
    "testSetGetOrdinalPosition",
    set_ordinal_position,
    ordinal_position,
    3
);

#[test]
fn set_get_char_octet_length() -> TestResult {
    let column = ColumnMeta::default();
    let original: Arc<dyn JavaObject> = Arc::new(255_i32);
    column.set_char_octet_length(Some(Arc::clone(&original)))?;
    let actual = column
        .char_octet_length()?
        .ok_or("Missing character length")?;
    assert!(original.java_equals(actual.as_ref())?);
    assert!(Arc::ptr_eq(&original, &actual));
    record(
        "testSetGetCharOctetLength",
        &[actual
            .java_to_string()?
            .ok_or("Missing object text")?
            .to_string()],
    )
}

#[test]
fn set_get_on_update() -> TestResult {
    let column = ColumnMeta::default();
    column.set_on_update(true)?;
    assert!(column.is_on_update()?);
    record(
        "testSetGetIsOnUpdate",
        &[column.is_on_update()?.to_string()],
    )
}

#[test]
fn set_get_case_sensitive() -> TestResult {
    let column = ColumnMeta::default();
    column.set_case_sensitive(true)?;
    assert!(column.is_case_sensitive()?);
    record(
        "testSetGetIsCaseSensitive",
        &[column.is_case_sensitive()?.to_string()],
    )
}
