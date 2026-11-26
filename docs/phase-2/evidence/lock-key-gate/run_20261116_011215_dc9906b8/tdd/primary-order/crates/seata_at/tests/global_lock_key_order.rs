//! 锁键使用原Java TableMeta的实际HashMap次序；SQL/镜像主键索引序保持独立。
#![allow(clippy::unwrap_used)]
use seata_at::{BaseTransactionalExecutor, MySqlJdbcProfile};
use seata_datasource::{ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, MySqlType, Row, TableMeta, TableRecords};

#[test]
fn java_lock_order_is_separate_from_database_index_and_physical_column_order() {
    let columns: Vec<_> = ["b", "a"].iter().enumerate().map(|(index,name)| {
        ColumnMeta::new((*name).into(),ColumnType::SignedInteger,false,index as u32+1,Some(index as u32+1),None,None,None)
            .unwrap().with_domain(ColumnDomain::Integer { bits:32,unsigned:false }).unwrap().with_mysql_type(MySqlType::Int).unwrap()
    }).collect();
    let table=TableMeta::new("composite".into(),columns).unwrap();
    let row=Row::new(&table,vec![Field::new(&table.columns()[0],FieldValue::Signed(2)).unwrap(),Field::new(&table.columns()[1],FieldValue::Signed(1)).unwrap()]).unwrap();
    assert_eq!(row.primary_keys().map(|field|field.column().name()).collect::<Vec<_>>(),vec!["b","a"]);
    let mut records=TableRecords::new(table);records.add(row).unwrap();
    assert_eq!(BaseTransactionalExecutor::build_lock_key(&records,MySqlJdbcProfile::Connector8NumericTinyInt).unwrap().as_deref(),Some("composite:1_2"));
}
