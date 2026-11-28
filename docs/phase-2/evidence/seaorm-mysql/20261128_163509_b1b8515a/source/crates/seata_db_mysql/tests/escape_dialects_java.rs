//! 十个原 Java 转义处理器的服务身份、关键字和 UTF-16 行为差分。
use seata_db_mysql as _;
use seata_sqlparser::{
    ColumnList, ColumnMeta, EscapeHandlerFactory, IndexMeta, IndexType, JavaString, MetadataError,
    MetadataLocale, TableMeta,
};
use std::{error::Error, sync::Arc};

fn decode(input: &str) -> Result<Option<JavaString>, Box<dyn Error>> {
    if input == "null" {
        return Ok(None);
    }
    let units = (0..input.len())
        .step_by(4)
        .map(|i| u16::from_str_radix(&input[i..i + 4], 16))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(JavaString::from_utf16(units)))
}

fn hex(value: Option<&JavaString>) -> String {
    let Some(value) = value else {
        return "null".into();
    };
    let mut text = String::with_capacity(value.as_utf16().len() * 4);
    for unit in value.as_utf16() {
        for shift in [12, 8, 4, 0] {
            text.push(char::from(
                b"0123456789abcdef"[usize::from((unit >> shift) & 15)],
            ));
        }
    }
    text
}

fn observe<T>(result: Result<T, MetadataError>, render: impl FnOnce(T) -> String) -> String {
    result.map_or_else(
        |error| {
            error
                .java_exception_class()
                .map_or_else(|| error.to_string(), str::to_owned)
        },
        render,
    )
}

#[test]
fn linked_escape_handlers_match_all_production_java_providers() -> Result<(), Box<dyn Error>> {
    let locale = MetadataLocale::new("en");
    let factory = EscapeHandlerFactory::discover(locale.clone())?;
    let mysql = factory.get_escape_handler(Some(&"mysql".into()))?;
    let mut count = 0;
    for row in include_str!("fixtures/escape-dialects-java-v3.tsv").lines() {
        let fields: Vec<_> = row.split('\t').collect();
        let [db, kind, ..] = fields.as_slice() else {
            return Err("missing observation fields".into());
        };
        let handler = factory.get_escape_handler(Some(&JavaString::from(*db)))?;
        let expected = fields.last().ok_or("missing result")?;
        let actual = match *kind {
            "identity" => {
                locale.set_language("en")?;
                let variant = factory.get_escape_handler(Some(&db.to_ascii_uppercase().into()))?;
                format!(
                    "{},{}",
                    Arc::ptr_eq(&handler, &variant),
                    Arc::ptr_eq(&handler, &mysql)
                )
            }
            "symbols" => {
                let Some(symbol) = handler.escape_symbol()? else {
                    return Err("missing escape symbol".into());
                };
                format!("{},{}", symbol.left_symbol(), symbol.right_symbol())
            }
            "primary-keys" => {
                locale.set_language("en")?;
                let table = TableMeta::new(locale.clone());
                let index = Arc::new(IndexMeta::default());
                index.set_indextype(Some(IndexType::PRIMARY))?;
                let mut values = Vec::new();
                for name in [
                    Some("select"),
                    Some("schema.Mixed"),
                    Some("UNCHANGED"),
                    None,
                ] {
                    let column = ColumnMeta::default();
                    column.set_column_name(name.map(JavaString::from))?;
                    values.push(Some(column));
                }
                index.set_values(Some(ColumnList::from_values(values)))?;
                table
                    .all_indexes()
                    .put(Some("primary".into()), Some(index))?;
                table
                    .escape_pk_name_list(Some(&JavaString::from(*db)), &factory)?
                    .iter()
                    .map(|name| hex(name.as_ref()))
                    .collect::<Vec<_>>()
                    .join(",")
            }
            "keyword" => {
                locale.set_language("en")?;
                let upper = JavaString::from(fields[2]);
                let lower = JavaString::from(fields[2].to_ascii_lowercase());
                format!(
                    "{},{}",
                    handler.check_if_key_words(Some(&upper))?,
                    handler.check_if_key_words(Some(&lower))?
                )
            }
            "name" => {
                locale.set_language(fields[2])?;
                let value = decode(fields[3])?;
                let input = value.as_ref();
                match fields[4] {
                    "add" => observe(handler.add_col_name_escape(input), |value| {
                        hex(Some(&value))
                    }),
                    "delete" => observe(handler.del_col_name_escape(input), |value| {
                        hex(value.as_ref())
                    }),
                    context => {
                        let meta = (context != "none").then(|| TableMeta::new(locale.clone()));
                        if let (Some(meta), Some(input), "sensitive" | "insensitive") =
                            (&meta, input, context)
                        {
                            let column = Arc::new(ColumnMeta::default());
                            column.set_case_sensitive(context == "sensitive")?;
                            meta.all_columns()
                                .put(Some(input.java_trim()), Some(column))?;
                        }
                        observe(
                            handler.check_if_need_escape(input, meta.as_ref()),
                            |value| value.to_string(),
                        )
                    }
                }
            }
            _ => return Err(format!("unknown Java observation: {row}").into()),
        };
        assert_eq!(&actual, expected, "{row:?}");
        count += 1;
    }
    assert_eq!(count, 5335);
    Ok(())
}
