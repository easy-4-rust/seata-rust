use super::DataCompareUtils;
use seata_datasource::{ColumnMeta, ColumnType, Field, FieldValue, Row, TableMeta, TableRecords};
use std::error::Error;

fn image(keys: &[(&str, &str)]) -> Result<TableRecords, Box<dyn Error>> {
    let table = TableMeta::new(
        "compare_rows".into(),
        ["id", "tenant"]
            .into_iter()
            .zip(1..)
            .map(|(name, i)| {
                ColumnMeta::new(
                    name.into(),
                    ColumnType::Text,
                    false,
                    i,
                    Some(i),
                    Some(32),
                    None,
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let mut records = TableRecords::new(table.clone());
    for &(id, tenant) in keys {
        let fields = table
            .columns()
            .iter()
            .zip([id, tenant])
            .map(|(column, value)| Field::new(column, FieldValue::Text(value.into())))
            .collect::<Result<Vec<_>, _>>()?;
        records.add(Row::new(&table, fields)?)?;
    }
    Ok(records)
}

#[test]
fn records_equal_rejects_a_repeated_key_in_either_image() -> Result<(), Box<dyn Error>> {
    let original = image(&[("first", "tenant"), ("second", "tenant")])?;
    let repeated = image(&[("first", "tenant"), ("first", "tenant")])?;
    assert!(
        !DataCompareUtils::records_equal(&original, &repeated),
        "right duplicate must not stand in for a missing primary key"
    );
    assert!(!DataCompareUtils::records_equal(&repeated, &original));
    assert!(!DataCompareUtils::records_equal(&repeated, &repeated));
    Ok(())
}

#[test]
fn records_equal_consumes_distinct_composite_keys_in_any_order() -> Result<(), Box<dyn Error>> {
    let original = image(&[("a_b", "c"), ("a", "b_c")])?;
    let reordered = image(&[("a", "b_c"), ("a_b", "c")])?;
    assert!(DataCompareUtils::records_equal(&original, &reordered));
    assert!(DataCompareUtils::records_equal(&reordered, &original));
    let missing_key = image(&[("a_b", "c")])?;
    let different_key = image(&[("a", "b_c")])?;
    assert!(!DataCompareUtils::records_equal(
        &missing_key,
        &different_key
    ));
    Ok(())
}
