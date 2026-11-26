//! 元数据类型的主键顺序、物理列序及无效输入契约。
#![allow(clippy::unwrap_used)]
use seata_datasource::{ColumnMeta, ColumnType, MetadataError, TableMeta};

fn column(name: &str, ordinal: u32, primary: Option<u32>) -> ColumnMeta {
    ColumnMeta::new(
        name.into(),
        ColumnType::SignedInteger,
        false,
        ordinal,
        primary,
        None,
        None,
        None,
    )
    .unwrap()
}

#[test]
fn primary_index_order_is_independent_of_physical_column_order() {
    let table = TableMeta::new(
        "Account".into(),
        vec![
            column("b", 1, Some(2)),
            column("a", 2, Some(1)),
            column("v", 3, None),
        ],
    )
    .unwrap();
    assert_eq!(table.name(), "Account");
    assert_eq!(
        table
            .columns()
            .iter()
            .map(ColumnMeta::name)
            .collect::<Vec<_>>(),
        ["b", "a", "v"]
    );
    assert_eq!(
        table
            .primary_key_columns()
            .map(ColumnMeta::name)
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(table.column("V").unwrap().name(), "v");
}

#[test]
fn incomplete_duplicate_or_nullable_primary_keys_are_rejected() {
    assert!(TableMeta::new("t".into(), vec![column("a", 1, None)]).is_err());
    assert!(TableMeta::new("t".into(), vec![column("a", 1, Some(2))]).is_err());
    assert!(TableMeta::new(
        "t".into(),
        vec![column("a", 1, Some(1)), column("b", 2, Some(1))]
    )
    .is_err());
    assert!(ColumnMeta::new(
        "id".into(),
        ColumnType::SignedInteger,
        true,
        1,
        Some(1),
        None,
        None,
        None
    )
    .is_err());
}

#[test]
fn column_names_and_ordinals_cannot_alias_or_silently_reorder() {
    for cols in [
        vec![column("id", 1, Some(1)), column("ID", 2, None)],
        vec![column("id", 2, Some(1)), column("v", 1, None)],
    ] {
        assert!(TableMeta::new("t".into(), cols).is_err());
    }
    assert!(matches!(
        TableMeta::new("bad`table".into(), vec![column("id", 1, Some(1))]),
        Err(MetadataError::Invalid { .. })
    ));
}

#[test]
fn decimal_metadata_and_schema_budgets_are_checked() {
    assert!(ColumnMeta::new(
        "v".into(),
        ColumnType::Decimal,
        false,
        1,
        None,
        None,
        Some(65),
        Some(30)
    )
    .is_ok());
    assert!(ColumnMeta::new(
        "v".into(),
        ColumnType::Decimal,
        false,
        1,
        None,
        None,
        Some(10),
        Some(11)
    )
    .is_err());
    assert!(ColumnMeta::new(
        "v".into(),
        ColumnType::Decimal,
        false,
        1,
        None,
        None,
        None,
        None
    )
    .is_err());
    let columns = (1..=129)
        .map(|i| column(&format!("c{i}"), i, if i == 1 { Some(1) } else { None }))
        .collect();
    assert!(TableMeta::new("t".into(), columns).is_err());
}
