//! MySQL物理类型维度不能被通用64位/字节值域覆盖。
#![allow(clippy::unwrap_used)]
use seata_datasource::{CharacterSet,ColumnDomain,ColumnMeta,ColumnType,DecimalValue,Field,FieldValue};
fn col(kind:ColumnType,length:Option<u64>)->ColumnMeta{
    ColumnMeta::new("v".into(),kind,true,1,None,length,(kind==ColumnType::Decimal).then_some(5),(kind==ColumnType::Decimal).then_some(2)).unwrap()
}
#[test]
fn signed_physical_widths_reject_one_past_each_boundary(){
    for bits in [8,16,24,32,64]{
        let c=col(ColumnType::SignedInteger,None).with_domain(ColumnDomain::Integer{bits,unsigned:false}).unwrap();
        let (min,max)=if bits==64{(i64::MIN,i64::MAX)}else{(-(1i64<<(bits-1)),(1i64<<(bits-1))-1)};
        for v in [min,max,0]{assert!(Field::new(&c,FieldValue::Signed(v)).is_ok());}
        if bits<64{for v in [min-1,max+1]{assert!(Field::new(&c,FieldValue::Signed(v)).is_err());}}
    }
}
#[test]
fn unsigned_physical_widths_do_not_wrap(){
    for bits in [8,16,24,32,64]{
        let c=col(ColumnType::UnsignedInteger,None).with_domain(ColumnDomain::Integer{bits,unsigned:true}).unwrap();
        let max=if bits==64{u64::MAX}else{(1u64<<bits)-1};
        assert!(Field::new(&c,FieldValue::Unsigned(max)).is_ok());
        if bits<64{assert!(Field::new(&c,FieldValue::Unsigned(max+1)).is_err());}
    }
}
#[test]
fn utf8_character_limit_is_not_an_octet_limit(){
    let c=col(ColumnType::Text,Some(4)).with_domain(ColumnDomain::Text{charset:CharacterSet::Utf8Mb4,character_limit:Some(1),fixed_width:false}).unwrap();
    for s in ["","中","🦀"]{assert!(Field::new(&c,FieldValue::Text(s.into())).is_ok());}
    assert!(Field::new(&c,FieldValue::Text("ab".into())).is_err());
}
#[test]
fn character_sets_reject_unrepresentable_values(){
    let ascii=col(ColumnType::Text,Some(8)).with_domain(ColumnDomain::Text{charset:CharacterSet::Ascii,character_limit:Some(8),fixed_width:false}).unwrap();
    assert!(Field::new(&ascii,FieldValue::Text("a\0".into())).is_ok());
    assert!(Field::new(&ascii,FieldValue::Text("é".into())).is_err());
    let mb3=col(ColumnType::Text,Some(6)).with_domain(ColumnDomain::Text{charset:CharacterSet::Utf8Mb3,character_limit:Some(2),fixed_width:false}).unwrap();
    assert!(Field::new(&mb3,FieldValue::Text("中文".into())).is_ok());
    assert!(Field::new(&mb3,FieldValue::Text("🦀".into())).is_err());
}
#[test]
fn text_lob_uses_octets_and_fixed_binary_requires_padding(){
    let text=col(ColumnType::Text,Some(4)).with_domain(ColumnDomain::Text{charset:CharacterSet::Utf8Mb4,character_limit:None,fixed_width:false}).unwrap();
    assert!(Field::new(&text,FieldValue::Text("abcd".into())).is_ok());
    let binary=col(ColumnType::Binary,Some(4)).with_domain(ColumnDomain::Binary{fixed_width:true}).unwrap();
    assert!(Field::new(&binary,FieldValue::Binary(vec![1,0,0,0])).is_ok());
    assert!(Field::new(&binary,FieldValue::Binary(vec![1])).is_err());
    assert!(Field::new(&binary,FieldValue::Null).is_ok());
}
#[test]
fn unsigned_decimal_rejects_negative_values(){
    let c=col(ColumnType::Decimal,None).with_domain(ColumnDomain::Decimal{unsigned:true}).unwrap();
    assert!(Field::new(&c,FieldValue::Decimal(DecimalValue::new("999.99").unwrap())).is_ok());
    assert!(Field::new(&c,FieldValue::Decimal(DecimalValue::new("-0.01").unwrap())).is_err());
}
#[test]
fn inconsistent_domain_metadata_is_rejected(){
    for domain in [ColumnDomain::Integer{bits:0,unsigned:false},ColumnDomain::Integer{bits:7,unsigned:false},ColumnDomain::Integer{bits:63,unsigned:false},ColumnDomain::Integer{bits:64,unsigned:true},ColumnDomain::Binary{fixed_width:false}]{
        assert!(col(ColumnType::SignedInteger,None).with_domain(domain).is_err());
    }
    for domain in [ColumnDomain::Text{charset:CharacterSet::Utf8Mb4,character_limit:Some(0),fixed_width:false},ColumnDomain::Text{charset:CharacterSet::Utf8Mb4,character_limit:Some(2),fixed_width:false},ColumnDomain::Text{charset:CharacterSet::Utf8Mb4,character_limit:None,fixed_width:true}]{
        assert!(col(ColumnType::Text,Some(4)).with_domain(domain).is_err());
    }
}
#[test]
fn legacy_metadata_remains_optional_and_domains_affect_schema_equality(){
    let c=col(ColumnType::SignedInteger,None);assert!(c.domain().is_none());
    assert!(Field::new(&c,FieldValue::Signed(128)).is_ok());
    let tiny=c.clone().with_domain(ColumnDomain::Integer{bits:8,unsigned:false}).unwrap();
    let wide=c.with_domain(ColumnDomain::Integer{bits:64,unsigned:false}).unwrap();assert_ne!(tiny,wide);
    assert!(Field::new(&tiny,FieldValue::Signed(128)).is_err());
}
