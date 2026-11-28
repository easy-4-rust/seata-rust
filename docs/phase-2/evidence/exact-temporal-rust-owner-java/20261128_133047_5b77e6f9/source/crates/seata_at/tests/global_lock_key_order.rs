//! 锁键使用原Java TableMeta的实际HashMap次序；SQL/镜像主键索引序保持独立。
#![allow(clippy::unwrap_used)]
use seata_at::{BaseTransactionalExecutor, MySqlJdbcProfile};
use seata_datasource::{
    ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, MySqlType, Row, TableMeta,
    TableRecords,
};

#[test]
fn java_lock_order_is_separate_from_database_index_and_physical_column_order() {
    let columns: Vec<_> = ["b", "a"]
        .iter()
        .enumerate()
        .map(|(index, name)| {
            ColumnMeta::new(
                (*name).into(),
                ColumnType::SignedInteger,
                false,
                index as u32 + 1,
                Some(index as u32 + 1),
                None,
                None,
                None,
            )
            .unwrap()
            .with_domain(ColumnDomain::Integer {
                bits: 32,
                unsigned: false,
            })
            .unwrap()
            .with_mysql_type(MySqlType::Int)
            .unwrap()
        })
        .collect();
    let table = TableMeta::new("composite".into(), columns).unwrap();
    let row = Row::new(
        &table,
        vec![
            Field::new(&table.columns()[0], FieldValue::Signed(2)).unwrap(),
            Field::new(&table.columns()[1], FieldValue::Signed(1)).unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(
        row.primary_keys()
            .map(|field| field.column().name())
            .collect::<Vec<_>>(),
        vec!["b", "a"]
    );
    let mut records = TableRecords::new(table);
    records.add(row).unwrap();
    assert_eq!(
        BaseTransactionalExecutor::build_lock_key(
            &records,
            MySqlJdbcProfile::Connector8NumericTinyInt
        )
        .unwrap()
        .as_deref(),
        Some("composite:1_2")
    );
}

#[test]
fn original_java_order_corpus_covers_resize_tree_collisions_and_split_iteration() {
    let cases: Vec<serde_json::Value> = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/locks/java_17/inputs.json"
    ))
    .unwrap();
    let observations: Vec<serde_json::Value> = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/locks/java_17/java_observations.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), observations.len());
    let mut checked = 0;
    for (case, original) in cases.iter().zip(&observations) {
        assert_eq!(case["case"], original["case"]);
        if case.get("types").is_some() {
            continue;
        }
        let names = case["names"].as_array().unwrap();
        let columns = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                ColumnMeta::new(
                    name.as_str().unwrap().into(),
                    ColumnType::SignedInteger,
                    false,
                    index as u32 + 1,
                    Some(index as u32 + 1),
                    None,
                    None,
                    None,
                )
                .unwrap()
                .with_domain(ColumnDomain::Integer {
                    bits: 32,
                    unsigned: false,
                })
                .unwrap()
                .with_mysql_type(MySqlType::Int)
                .unwrap()
            })
            .collect();
        let table = TableMeta::new(case["table"].as_str().unwrap().into(), columns).unwrap();
        let mut records = TableRecords::new(table.clone());
        for raw in case["rows"].as_array().unwrap() {
            let fields = table
                .columns()
                .iter()
                .zip(raw.as_array().unwrap())
                .map(|(column, value)| {
                    Field::new(column, FieldValue::Signed(value.as_i64().unwrap())).unwrap()
                })
                .collect();
            records.add(Row::new(&table, fields).unwrap()).unwrap();
        }
        let actual = BaseTransactionalExecutor::build_lock_key(
            &records,
            MySqlJdbcProfile::Connector8NumericTinyInt,
        )
        .unwrap();
        assert_eq!(
            actual.as_deref(),
            original["lock_key"].as_str(),
            "original Java order case {}",
            case["case"]
        );
        checked += 1;
    }
    assert_eq!(checked, 1003);
}
