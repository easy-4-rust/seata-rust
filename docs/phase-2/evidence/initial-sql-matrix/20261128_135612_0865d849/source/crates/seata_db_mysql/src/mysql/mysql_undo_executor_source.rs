//! 三类原 Undo 测试共用的原输入和断言记录，非生产执行器替身。
use seata_at::{SqlType, SqlUndoLog};
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, ColumnType, Field, FieldValue, MySqlType, Row,
    TableMeta, TableRecords,
};
use serde_json::Value;
use std::{error::Error, fs::OpenOptions, io::Write, sync::Mutex};

static OBSERVATIONS: Mutex<()> = Mutex::new(());

pub(super) fn original_log() -> Result<SqlUndoLog, Box<dyn Error>> {
    let input: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/mysql_undo_executor_source.json"
    ))?;
    assert_eq!(input["sql_type"], "UPDATE"); // 三个原类均传 UPDATE，不能按执行器名称修正原输入。
    let columns = input["columns"].as_array().ok_or("original columns")?;
    let mut metadata = Vec::new();
    for (index, column) in columns.iter().enumerate() {
        assert_eq!(column["jdbc_type"], 1); // java.sql.Types.CHAR 精确映射到原生 Char，而非 VARCHAR。
        let name = column["name"].as_str().ok_or("original column name")?;
        let primary = input["primary_keys"]
            .as_array()
            .ok_or("original PK")?
            .iter()
            .position(|key| key.as_str() == Some(name));
        assert_eq!(
            column["key_type"],
            if primary.is_some() {
                "PRIMARY_KEY"
            } else {
                "NULL"
            }
        );
        metadata.push(
            ColumnMeta::new(
                name.into(),
                ColumnType::Text,
                false,
                u32::try_from(index + 1)?,
                primary.map(|p| u32::try_from(p + 1)).transpose()?,
                Some(64),
                None,
                None,
            )?
            .with_domain(ColumnDomain::Text {
                charset: CharacterSet::Ascii,
                character_limit: Some(64),
                fixed_width: true,
            })?
            .with_mysql_type(MySqlType::Char)?,
        );
    }
    // 原 TableMeta mock 只给表名和 PK；物理模型所需维度仅作测试装箱，不冒充原 mock 已提供它们。
    let table = TableMeta::new(
        input["table_name"].as_str().ok_or("original table")?.into(),
        metadata,
    )?;
    let build = |key: &str| -> Result<TableRecords, Box<dyn Error>> {
        let mut records = TableRecords::new(table.clone());
        for row in input[key].as_array().ok_or("original rows")? {
            let values = row.as_array().ok_or("original row")?;
            assert_eq!(values.len(), table.columns().len());
            let fields = table
                .columns()
                .iter()
                .zip(values)
                .map(|(column, value)| {
                    assert_eq!(column.mysql_type(), Some(MySqlType::Char));
                    Field::new(
                        column,
                        FieldValue::Text(value.as_str().ok_or("original CHAR value")?.into()),
                    )
                    .map_err(Into::into)
                })
                .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
            records.add(Row::new(&table, fields)?)?;
        }
        Ok(records)
    };
    Ok(SqlUndoLog::new(
        SqlType::Update,
        build("before_image")?,
        build("after_image")?,
    )?)
}

pub(super) fn record(class: &str, method: &str, values: &[String]) -> Result<(), Box<dyn Error>> {
    let Some(path) = std::env::var_os("SEATA_MYSQL_UNDO_EXECUTOR_OBSERVATIONS") else {
        return Ok(());
    };
    let _guard = OBSERVATIONS
        .lock()
        .map_err(|_| "observation lock poisoned")?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let bytes = values.join("\u{1f}").into_bytes();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let encoded = bytes
        .iter()
        .flat_map(|byte| {
            [
                char::from(HEX[usize::from(byte >> 4)]),
                char::from(HEX[usize::from(byte & 15)]),
            ]
        })
        .collect::<String>();
    writeln!(
        file,
        "org.apache.seata.rm.datasource.undo.mysql.{class}#{method}\t{encoded}"
    )?;
    Ok(())
}
