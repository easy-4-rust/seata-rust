//! 自增属性仅允许明确的非空整数单列主键，旧元数据构造兼容。
#![allow(clippy::unwrap_used)]
use seata_datasource::{ColumnMeta,ColumnType,TableMeta};
fn integer(name:&str,position:u32,primary:Option<u32>)->ColumnMeta {
    ColumnMeta::new(name.into(),ColumnType::SignedInteger,false,position,primary,None,None,None).unwrap()
}
#[test]
fn legacy_metadata_defaults_to_non_auto_and_only_primary_integer_can_enable_it() {
    let old=integer("id",1,Some(1));assert!(!old.is_auto_increment());
    assert!(old.with_auto_increment(true).unwrap().is_auto_increment());
    assert!(integer("v",1,None).with_auto_increment(true).is_err());
    assert!(integer("id",1,Some(2)).with_auto_increment(true).is_err());
    let text=ColumnMeta::new("id".into(),ColumnType::Text,false,1,Some(1),Some(16),None,None).unwrap();
    assert!(text.with_auto_increment(true).is_err());
}
#[test]
fn a_composite_primary_key_cannot_claim_single_generated_key_semantics() {
    let first=integer("id",1,Some(1)).with_auto_increment(true).unwrap();
    assert!(TableMeta::new("items".into(),vec![first.clone(),integer("v",2,Some(2))]).is_err());
    assert!(TableMeta::new("items".into(),vec![first,integer("v",2,None)]).is_ok());
}
