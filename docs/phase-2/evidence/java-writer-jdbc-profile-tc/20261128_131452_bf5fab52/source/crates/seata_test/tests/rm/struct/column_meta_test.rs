//! 对应 Java: org.apache.seata.rm.datasource.sql.struct.ColumnMetaTest；保留全部原断言与顺序。
#![allow(clippy::unwrap_used)]
#[path = "../../support/rm_metadata_source.rs"]
#[allow(dead_code)]
mod support;
use seata_sqlparser::ColumnMeta;
use std::sync::Arc;
use support::{check, observe, TestResult};

#[test]
fn original_rm_test_column_meta() -> TestResult {
    let c = ColumnMeta::default();
    let mut values = Vec::new();
    check(&mut values, "not_null", c.java_to_string().is_ok(), true);
    check(
        &mut values,
        "equals:object",
        c.java_equals(Some(&ColumnMeta::default()))?,
        true,
    );
    c.set_is_autoincrement(Some("Yes".into()))?;
    check(&mut values, "true", c.is_autoincrement()?, true);
    macro_rules! other {
        ($setter:ident,$value:expr) => {
            let other = ColumnMeta::default();
            other.$setter($value)?;
            check(
                &mut values,
                "not_equals",
                !c.java_equals(Some(&other))?,
                true,
            );
        };
    }
    other!(set_table_cat, Some("".into()));
    other!(set_table_schema_name, Some("".into()));
    other!(set_table_name, Some("".into()));
    other!(set_column_name, Some("".into()));
    other!(set_data_type, 1);
    other!(set_data_type_name, Some("".into()));
    other!(set_column_size, 1);
    other!(set_decimal_digits, 1);
    other!(set_num_prec_radix, 1);
    other!(set_null_able, 1);
    other!(set_remarks, Some("".into()));
    other!(set_column_def, Some("".into()));
    other!(set_sql_data_type, 1);
    other!(set_sql_datetime_sub, 1);
    other!(set_char_octet_length, Some(Arc::new(1_i32)));
    other!(set_ordinal_position, 1);
    other!(set_is_null_able, Some("".into()));
    other!(set_is_autoincrement, Some("".into()));
    observe("ColumnMetaTest", "testColumnMeta", &values)
}
