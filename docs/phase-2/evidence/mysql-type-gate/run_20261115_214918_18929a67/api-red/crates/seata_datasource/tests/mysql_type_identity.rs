//! 原生类型身份不能被通用值域吞并；旧元数据构造保持未提供身份。
#![allow(clippy::unwrap_used)]
use seata_datasource::{CharacterSet, ColumnDomain, ColumnMeta, ColumnType, MySqlType};

fn integer(bits:u8)->ColumnMeta {
    ColumnMeta::new("v".into(),ColumnType::SignedInteger,false,1,Some(1),None,None,None).unwrap()
        .with_domain(ColumnDomain::Integer {bits,unsigned:false}).unwrap()
}
#[test]
fn legacy_constructor_does_not_guess_a_native_type() {
    assert_eq!(integer(8).mysql_type(),None);
}
#[test]
fn integer_width_and_boolean_display_identity_remain_distinct() {
    for (bits,kind) in [(8,MySqlType::TinyInt {display_width_one:false}),(16,MySqlType::SmallInt),
        (24,MySqlType::MediumInt),(32,MySqlType::Int),(64,MySqlType::BigInt)] {
        let column=integer(bits).with_mysql_type(kind).unwrap();
        assert_eq!(column.mysql_type(),Some(kind));
        assert!(integer(if bits==8 {16} else {8}).with_mysql_type(kind).is_err());
    }
    let boolean=integer(8).with_mysql_type(MySqlType::TinyInt{display_width_one:true}).unwrap();
    let numeric=integer(8).with_mysql_type(MySqlType::TinyInt{display_width_one:false}).unwrap();
    assert_ne!(boolean,numeric);
}
#[test]
fn changing_domain_cannot_invalidate_existing_native_identity() {
    let column=integer(8).with_mysql_type(MySqlType::TinyInt{display_width_one:false}).unwrap();
    assert!(column.with_domain(ColumnDomain::Integer{bits:16,unsigned:false}).is_err());
}
#[test]
fn native_identity_requires_a_real_compatible_domain() {
    let unqualified=ColumnMeta::new("v".into(),ColumnType::SignedInteger,false,1,Some(1),None,None,None).unwrap();
    assert!(unqualified.with_mysql_type(MySqlType::Int).is_err());
    assert!(integer(32).with_mysql_type(MySqlType::VarChar).is_err());
}
#[test]
fn text_subtypes_keep_jdbc_relevant_lob_identity() {
    let text=ColumnMeta::new("v".into(),ColumnType::Text,true,1,None,Some(1024),None,None).unwrap()
        .with_domain(ColumnDomain::Text{charset:CharacterSet::Utf8Mb4,character_limit:None,fixed_width:false}).unwrap();
    for kind in [MySqlType::TinyText,MySqlType::Text,MySqlType::MediumText,MySqlType::LongText] {
        assert_eq!(text.clone().with_mysql_type(kind).unwrap().mysql_type(),Some(kind));
    }
    assert!(text.with_mysql_type(MySqlType::VarChar).is_err());
    let varchar=ColumnMeta::new("v".into(),ColumnType::Text,true,1,None,Some(12),None,None).unwrap()
        .with_domain(ColumnDomain::Text{charset:CharacterSet::Utf8Mb4,character_limit:Some(3),fixed_width:false}).unwrap();
    assert!(varchar.clone().with_mysql_type(MySqlType::VarChar).is_ok());
    assert!(varchar.with_mysql_type(MySqlType::Char).is_err());
}
#[test]
fn binary_and_blob_subtypes_are_not_conflated() {
    let column=ColumnMeta::new("v".into(),ColumnType::Binary,true,1,None,Some(32),None,None).unwrap()
        .with_domain(ColumnDomain::Binary{fixed_width:false}).unwrap();
    for kind in [MySqlType::VarBinary,MySqlType::TinyBlob,MySqlType::Blob,MySqlType::MediumBlob,MySqlType::LongBlob] {
        assert_eq!(column.clone().with_mysql_type(kind).unwrap().mysql_type(),Some(kind));
    }
    assert!(column.with_mysql_type(MySqlType::Binary).is_err());
}
