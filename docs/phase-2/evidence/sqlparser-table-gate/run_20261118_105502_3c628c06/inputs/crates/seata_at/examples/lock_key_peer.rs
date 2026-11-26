//! 固定自有主键名称序列的真实Rust锁键输出，交由原Java方法逐例比较。
#![allow(clippy::unwrap_used)]
use seata_at::{BaseTransactionalExecutor, MySqlJdbcProfile};
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, ColumnType, DecimalValue, Field, FieldValue, MySqlType,
    Row, TableMeta, TableRecords,
};
use serde_json::{json, Value};
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("input missing")?);
    let output = PathBuf::from(args.next().ok_or("output missing")?);
    let cases: Vec<Value> = serde_json::from_slice(&fs::read(input)?)?;
    let mut result = Vec::new();
    for mut case in cases {
        let names = case["names"].as_array().ok_or("owned names missing")?;
        let columns = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                column(
                    name.as_str().unwrap(),
                    index,
                    case["types"][index].as_str().unwrap_or("integer"),
                    &case["rows"][0][index],
                )
            })
            .collect();
        let table = TableMeta::new(
            case["table"].as_str().ok_or("owned table missing")?.into(),
            columns,
        )?;
        let mut records = TableRecords::new(table.clone());
        for raw in case["rows"].as_array().ok_or("owned rows missing")? {
            let values = raw.as_array().ok_or("owned row array")?;
            let fields = table
                .columns()
                .iter()
                .zip(values)
                .map(|(column, raw)| Field::new(column, value(raw)).unwrap())
                .collect();
            records.add(Row::new(&table, fields)?)?;
        }
        let profile = if case["profile"] == "default" {
            MySqlJdbcProfile::Connector8Default
        } else {
            MySqlJdbcProfile::Connector8NumericTinyInt
        };
        let key = BaseTransactionalExecutor::build_lock_key(&records, profile);
        case["candidate_rejected"] = json!(key.is_err());
        case["candidate"] = json!(key.ok().flatten());
        result.push(case);
    }
    fs::write(output, serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}

fn column(name: &str, index: usize, kind: &str, sample: &Value) -> ColumnMeta {
    let (category, domain, native, octets, precision, scale) = match kind {
        "text" => (
            ColumnType::Text,
            ColumnDomain::Text {
                charset: CharacterSet::Utf8Mb4,
                character_limit: Some(1024),
                fixed_width: false,
            },
            MySqlType::VarChar,
            Some(4096),
            None,
            None,
        ),
        "binary" => (
            ColumnType::Binary,
            ColumnDomain::Binary { fixed_width: false },
            MySqlType::VarBinary,
            Some(4096),
            None,
            None,
        ),
        "decimal" => (
            ColumnType::Decimal,
            ColumnDomain::Decimal { unsigned: false },
            MySqlType::Decimal,
            None,
            Some(65),
            Some(
                DecimalValue::new(sample["text"].as_str().unwrap())
                    .unwrap()
                    .scale(),
            ),
        ),
        "unsigned" => (
            ColumnType::UnsignedInteger,
            ColumnDomain::Integer {
                bits: 64,
                unsigned: true,
            },
            MySqlType::BigInt,
            None,
            None,
            None,
        ),
        "signed64" => (
            ColumnType::SignedInteger,
            ColumnDomain::Integer {
                bits: 64,
                unsigned: false,
            },
            MySqlType::BigInt,
            None,
            None,
            None,
        ),
        "tiny" => (
            ColumnType::SignedInteger,
            ColumnDomain::Integer {
                bits: 8,
                unsigned: false,
            },
            MySqlType::TinyInt {
                display_width_one: true,
            },
            None,
            None,
            None,
        ),
        _ => (
            ColumnType::SignedInteger,
            ColumnDomain::Integer {
                bits: 32,
                unsigned: false,
            },
            MySqlType::Int,
            None,
            None,
            None,
        ),
    };
    ColumnMeta::new(
        name.into(),
        category,
        false,
        index as u32 + 1,
        Some(index as u32 + 1),
        octets,
        precision,
        scale,
    )
    .unwrap()
    .with_domain(domain)
    .unwrap()
    .with_mysql_type(native)
    .unwrap()
}
fn value(raw: &Value) -> FieldValue {
    if let Some(text) = raw.as_str() {
        return FieldValue::Text(text.into());
    }
    if raw.is_object() {
        let text = raw["text"].as_str().unwrap();
        return match raw["kind"].as_str().unwrap() {
            "decimal" => FieldValue::Decimal(DecimalValue::new(text).unwrap()),
            "bigint" => FieldValue::Unsigned(text.parse().unwrap()),
            "binary" => FieldValue::Binary(
                text.as_bytes()
                    .chunks_exact(2)
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect(),
            ),
            "boolean" => FieldValue::Signed(i64::from(text == "true")),
            _ => panic!("owned fixture value kind"),
        };
    }
    FieldValue::Signed(raw.as_i64().unwrap())
}
