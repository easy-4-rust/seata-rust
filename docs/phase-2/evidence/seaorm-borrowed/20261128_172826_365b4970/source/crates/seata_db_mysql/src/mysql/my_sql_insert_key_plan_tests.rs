//! 生成键计划只使用原回执；零/NULL 与批量拒绝在业务执行前分类。
#![allow(clippy::unwrap_used)]
use super::my_sql_insert_key_plan::MySqlInsertKeyPlan;
use crate::MySqlStatementResult;
use seata_datasource::{ColumnDomain, ColumnMeta, ColumnType, FieldValue, TableMeta};
fn table(unsigned: bool, bits: u8) -> TableMeta {
    TableMeta::new(
        "items".into(),
        vec![ColumnMeta::new(
            "id".into(),
            if unsigned {
                ColumnType::UnsignedInteger
            } else {
                ColumnType::SignedInteger
            },
            false,
            1,
            Some(1),
            None,
            None,
            None,
        )
        .unwrap()
        .with_domain(ColumnDomain::Integer { bits, unsigned })
        .unwrap()
        .with_auto_increment(true)
        .unwrap()],
    )
    .unwrap()
}
#[test]
fn zero_mode_and_null_control_generation_without_inventing_batch_ids() {
    let zero = vec![vec![FieldValue::Unsigned(0)]];
    assert!(matches!(
        MySqlInsertKeyPlan::assigned(zero.clone(), true, false).unwrap(),
        MySqlInsertKeyPlan::Generated
    ));
    assert!(matches!(
        MySqlInsertKeyPlan::assigned(zero, true, true).unwrap(),
        MySqlInsertKeyPlan::Explicit(_)
    ));
    assert!(matches!(
        MySqlInsertKeyPlan::assigned(vec![vec![FieldValue::Null]], true, true).unwrap(),
        MySqlInsertKeyPlan::Generated
    ));
    assert!(MySqlInsertKeyPlan::assigned(
        vec![vec![FieldValue::Null], vec![FieldValue::Unsigned(200)]],
        true,
        false
    )
    .is_err());
    assert!(MySqlInsertKeyPlan::generated(2).is_err());
}
#[test]
fn original_unsigned_id_is_lossless_and_count_zero_and_domain_overflow_are_rejected() {
    let id = 9223372036854775811;
    assert_eq!(
        MySqlInsertKeyPlan::Generated
            .resolve(MySqlStatementResult::new(1, id), &table(true, 64))
            .unwrap(),
        vec![vec![FieldValue::Unsigned(id)]]
    );
    assert!(MySqlInsertKeyPlan::Generated
        .resolve(MySqlStatementResult::new(1, id), &table(false, 64))
        .is_err());
    assert!(MySqlInsertKeyPlan::Generated
        .resolve(MySqlStatementResult::new(1, 256), &table(true, 8))
        .is_err());
    assert!(MySqlInsertKeyPlan::Generated
        .resolve(MySqlStatementResult::new(1, 0), &table(true, 64))
        .is_err());
    assert!(MySqlInsertKeyPlan::Generated
        .resolve(MySqlStatementResult::new(2, 100), &table(true, 64))
        .is_err());
}
