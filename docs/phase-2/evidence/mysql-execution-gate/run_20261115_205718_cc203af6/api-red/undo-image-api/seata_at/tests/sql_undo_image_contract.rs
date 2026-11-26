//! 事务内Undo图像的形状/身份检查，不是Java序列化或已持久化证明。
#![allow(clippy::unwrap_used)]
use seata_at::{SqlType,SqlUndoLog};
use seata_datasource::{ColumnMeta,ColumnType,Field,FieldValue,Row,TableMeta,TableRecords};

fn meta(name:&str)->TableMeta {
    TableMeta::new(name.into(),vec![ColumnMeta::new("id".into(),ColumnType::SignedInteger,false,1,Some(1),None,None,None).unwrap(),
        ColumnMeta::new("value".into(),ColumnType::Text,true,2,None,Some(64),None,None).unwrap()]).unwrap()
}
fn records(table:&TableMeta,values:&[(i64,&str)])->TableRecords {
    let mut records=TableRecords::new(table.clone());
    for(id,text)in values{
        records.add(Row::new(table,vec![Field::new(&table.columns()[0],FieldValue::Signed(*id)).unwrap(),
            Field::new(&table.columns()[1],FieldValue::Text((*text).into())).unwrap()]).unwrap()).unwrap();
    }
    records
}
#[test]
fn valid_insert_update_delete_keep_full_schema_and_exact_values(){
    let table=meta("t");
    let insert=SqlUndoLog::new(SqlType::Insert,records(&table,&[]),records(&table,&[(1,"x")])).unwrap();
    assert_eq!(insert.sql_type(),SqlType::Insert);assert_eq!(insert.table_name(),"t");assert_eq!(insert.value_bytes(),9);
    assert!(insert.before_image().rows().is_empty());assert_eq!(insert.after_image().rows().len(),1);
    let update=SqlUndoLog::new(SqlType::Update,records(&table,&[(1,"old")]),records(&table,&[(1,"new")])).unwrap();assert_eq!(update.value_bytes(),22);
    let delete=SqlUndoLog::new(SqlType::Delete,records(&table,&[(1,"x")]),records(&table,&[])).unwrap();assert!(delete.after_image().rows().is_empty());
}
#[test]
fn insert_and_delete_require_the_correct_empty_side(){
    let table=meta("t");
    assert!(SqlUndoLog::new(SqlType::Insert,records(&table,&[(1,"x")]),records(&table,&[(2,"y")])).is_err());
    assert!(SqlUndoLog::new(SqlType::Insert,records(&table,&[]),records(&table,&[])).is_err());
    assert!(SqlUndoLog::new(SqlType::Delete,records(&table,&[(1,"x")]),records(&table,&[(1,"x")])).is_err());
}
#[test]
fn update_identity_order_and_schema_cannot_change(){
    let table=meta("t");let other=meta("other");
    assert!(SqlUndoLog::new(SqlType::Update,records(&table,&[(1,"x")]),records(&table,&[(2,"x")])).is_err());
    assert!(SqlUndoLog::new(SqlType::Update,records(&table,&[(1,"x"),(2,"y")]),records(&table,&[(2,"y"),(1,"x")])).is_err());
    assert!(SqlUndoLog::new(SqlType::Update,records(&table,&[(1,"x")]),records(&other,&[(1,"x")])).is_err());
}
#[test]
fn duplicate_raw_primary_identities_are_rejected(){
    let table=meta("t");
    assert!(SqlUndoLog::new(SqlType::Insert,records(&table,&[]),records(&table,&[(1,"x"),(1,"y")])).is_err());
    assert!(SqlUndoLog::new(SqlType::Delete,records(&table,&[(1,"x"),(1,"y")]),records(&table,&[])).is_err());
}
#[test]
fn empty_update_and_delete_are_valid_no_match_images(){
    let table=meta("t");
    for kind in [SqlType::Update,SqlType::Delete]{assert!(SqlUndoLog::new(kind,records(&table,&[]),records(&table,&[])).is_ok());}
}
#[test]
fn undo_debug_does_not_disclose_values(){
    let table=meta("t");let log=SqlUndoLog::new(SqlType::Update,records(&table,&[(i64::MAX,"private-cell-123")]),records(&table,&[(i64::MAX,"private-cell-456")])).unwrap();
    let debug=format!("{log:?}");assert!(!debug.contains("private-cell"));assert!(!debug.contains(&i64::MAX.to_string()));
}
