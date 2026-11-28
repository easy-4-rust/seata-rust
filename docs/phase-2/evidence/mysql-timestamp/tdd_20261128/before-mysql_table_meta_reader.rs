use crate::DbError;
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, ColumnType, MetadataError, MySqlType, TableMeta,
};
use sqlx::{mysql::MySqlRow, Column, Executor, MySql, Row, Transaction};
use std::collections::HashMap;

/// 持有同事务MDL时读取MySQL真实表元数据，不以跨事务缓存替代当前数据库事实。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.cache.MysqlTableMetaCache#fetchSchema/resultSetMetaToSchema（部分实现）。
pub(crate) struct MySqlTableMetaReader;
impl MySqlTableMetaReader {
    /// 参数为持有目标表 MDL 的原事务与表名；从当前物理建表定义核对 InnoDB，返回校验结果。
    /// 对应 Java: 无直接方法；Rust 跨表字典快照下的事务引擎校验。
    pub(crate) async fn verify_current_engine(
        tx: &mut Transaction<'_, MySql>,
        table: &str,
    ) -> Result<(), DbError> {
        let quoted = TableMeta::quote_identifier(table)?;
        let row = sqlx::query(&format!("SHOW CREATE TABLE {quoted}"))
            .fetch_one(&mut **tx)
            .await
            .map_err(|error| DbError::driver("current transactional engine", error))?;
        let bytes: &[u8] = row
            .try_get(1)
            .map_err(|error| DbError::driver("current transactional engine decode", error))?;
        let definition = std::str::from_utf8(bytes).map_err(|_| DbError::Unsupported {
            feature: "physical table definition encoding",
        })?;
        Self::verify_engine_definition(definition)
    }

    /// 参数为当前 SHOW CREATE TABLE 原定义；仅确认 InnoDB 物理基表，返回固定校验错误。
    pub(crate) fn verify_engine_definition(definition: &str) -> Result<(), DbError> {
        let engine = definition.lines().find_map(|line| {
            line.strip_prefix(") ENGINE=")
                .and_then(|options| options.split_ascii_whitespace().next())
        });
        if engine != Some("InnoDB") {
            return Err(DbError::Unsupported {
                feature: "current physical engine must be InnoDB",
            });
        }
        Ok(())
    }
    /// 参数为原独占事务、schema与表名；返回精确类型/列序/PK序；未知或不可见形状拒绝。
    pub(crate) async fn fetch(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        table: &str,
    ) -> Result<TableMeta, DbError> {
        let quoted = TableMeta::quote_identifier(table)?;
        if table.eq_ignore_ascii_case("undo_log")
            || table.eq_ignore_ascii_case("seata_rs_at_branch_v1")
        {
            return Err(DbError::Unsupported {
                feature: "undo table as business resource",
            });
        }
        sqlx::query(&format!("SELECT 1 FROM {quoted} LIMIT 0 FOR UPDATE"))
            .execute(&mut **tx)
            .await
            .map_err(|error| DbError::driver("metadata lock", error))?;
        let row=sqlx::query("SELECT CAST(TABLE_TYPE AS CHAR CHARACTER SET utf8mb4) AS TABLE_TYPE, CAST(ENGINE AS CHAR CHARACTER SET utf8mb4) AS ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=? AND TABLE_NAME=?")
            .bind(database).bind(table).fetch_optional(&mut **tx).await.map_err(|error|DbError::driver("table metadata",error))?
            .ok_or(DbError::Unsupported { feature: "missing/invisible table" })?;
        let table_type: String = row
            .try_get("TABLE_TYPE")
            .map_err(|e| DbError::driver("table kind", e))?;
        let engine: Option<String> = row
            .try_get("ENGINE")
            .map_err(|e| DbError::driver("table engine", e))?;
        if table_type != "BASE TABLE"
            || !engine
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("InnoDB"))
        {
            return Err(DbError::Unsupported {
                feature: "InnoDB base table required",
            });
        }
        let (has_trigger, has_foreign_key) =
            crate::current_table_constraints::CurrentTableConstraints::read(tx, database, table)
                .await?;
        if has_trigger {
            return Err(DbError::Unsupported {
                feature: "table trigger",
            });
        }
        if has_foreign_key {
            return Err(DbError::Unsupported {
                feature: "inbound/outbound foreign key",
            });
        }
        let primary=sqlx::query("SELECT CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4) AS COLUMN_NAME, CAST(SEQ_IN_INDEX AS UNSIGNED) AS SEQ_IN_INDEX, CAST(SUB_PART AS UNSIGNED) AS SUB_PART FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? AND INDEX_NAME='PRIMARY' ORDER BY SEQ_IN_INDEX LIMIT 129")
            .bind(database).bind(table).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("primary metadata",e))?;
        let mut primary_order = HashMap::new();
        for row in primary {
            let name: Option<String> = row
                .try_get("COLUMN_NAME")
                .map_err(|e| DbError::driver("primary name", e))?;
            let name = name.ok_or(DbError::Unsupported {
                feature: "functional primary key",
            })?;
            let prefix: Option<u64> = row
                .try_get("SUB_PART")
                .map_err(|e| DbError::driver("primary prefix", e))?;
            if prefix.is_some() {
                return Err(DbError::Unsupported {
                    feature: "prefix primary key",
                });
            }
            let position: u64 = row
                .try_get("SEQ_IN_INDEX")
                .map_err(|e| DbError::driver("primary order", e))?;
            primary_order.insert(name.to_ascii_lowercase(), Self::u32(position)?);
        }
        let columns=sqlx::query("SELECT CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4) AS COLUMN_NAME, CAST(DATA_TYPE AS CHAR CHARACTER SET utf8mb4) AS DATA_TYPE, CAST(COLUMN_TYPE AS CHAR CHARACTER SET utf8mb4) AS COLUMN_TYPE, CAST(IS_NULLABLE AS CHAR CHARACTER SET utf8mb4) AS IS_NULLABLE, CAST(ORDINAL_POSITION AS UNSIGNED) AS ORDINAL_POSITION, CAST(CHARACTER_OCTET_LENGTH AS UNSIGNED) AS CHARACTER_OCTET_LENGTH, CAST(CHARACTER_MAXIMUM_LENGTH AS UNSIGNED) AS CHARACTER_MAXIMUM_LENGTH, CAST(NUMERIC_PRECISION AS UNSIGNED) AS NUMERIC_PRECISION, CAST(NUMERIC_SCALE AS UNSIGNED) AS NUMERIC_SCALE, CAST(DATETIME_PRECISION AS UNSIGNED) AS DATETIME_PRECISION, CAST(EXTRA AS CHAR CHARACTER SET utf8mb4) AS EXTRA, CAST(GENERATION_EXPRESSION AS CHAR CHARACTER SET utf8mb4) AS GENERATION_EXPRESSION, CAST(CHARACTER_SET_NAME AS CHAR CHARACTER SET utf8mb4) AS CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY ORDINAL_POSITION LIMIT 129")
            .bind(database).bind(table).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("column metadata",e))?;
        if columns.len() > 128 {
            return Err(MetadataError::Budget {
                limit: "1..128 columns",
            }
            .into());
        }
        Self::verify_physical_primary(tx, &quoted, &primary_order, &columns).await?;
        let columns = columns
            .iter()
            .map(|row| Self::column(row, &primary_order))
            .collect::<Result<Vec<_>, _>>()?;
        TableMeta::new(table.into(), columns).map_err(DbError::from)
    }
    /// 参数为持锁表与字典主键顺序；与服务器当前物理建表定义逐项核对。
    async fn verify_physical_primary(
        tx: &mut Transaction<'static, MySql>,
        quoted: &str,
        primary_order: &HashMap<String, u32>,
        columns: &[MySqlRow],
    ) -> Result<(), DbError> {
        // PREPARE 的结果列来自当前已解析的物理表，空结果也保留完整列序及可空性。
        let shape = (&mut **tx)
            .describe(&format!("SELECT * FROM {quoted} LIMIT 0"))
            .await
            .map_err(|error| DbError::driver("current physical result columns", error))?;
        if shape.columns().len() != columns.len() {
            return Err(DbError::Unsupported {
                feature: "table definition changed after transaction metadata snapshot",
            });
        }
        for (position, (actual, snapshot)) in shape.columns().iter().zip(columns).enumerate() {
            let name: String = snapshot
                .try_get("COLUMN_NAME")
                .map_err(|error| DbError::driver("snapshot result column name", error))?;
            let nullable: String = snapshot
                .try_get("IS_NULLABLE")
                .map_err(|error| DbError::driver("snapshot result column nullability", error))?;
            if actual.name() != name || shape.nullable(position) != Some(nullable == "YES") {
                return Err(DbError::Unsupported {
                    feature: "table definition changed after transaction metadata snapshot",
                });
            }
        }
        let row = sqlx::query(&format!("SHOW CREATE TABLE {quoted}"))
            .fetch_one(&mut **tx)
            .await
            .map_err(|error| DbError::driver("physical primary definition", error))?;
        let bytes: &[u8] = row
            .try_get(1)
            .map_err(|error| DbError::driver("physical primary definition decode", error))?;
        let definition = std::str::from_utf8(bytes).map_err(|_| DbError::Unsupported {
            feature: "physical primary definition encoding",
        })?;
        // SELECT * 不包含不可见列，另外核对服务器标准建表输出中的实际列数量。
        let physical_columns = definition
            .lines()
            .filter(|line| line.starts_with("  "))
            .filter(|line| line.trim_start().starts_with('`'))
            .count();
        if physical_columns != columns.len() {
            return Err(DbError::Unsupported {
                feature: "table definition changed after transaction metadata snapshot",
            });
        }
        // 当前物理定义的表选项独立于旧字典快照。仅接受明确的事务引擎。
        Self::verify_engine_definition(definition)?;
        for column in columns {
            let name: String = column
                .try_get("COLUMN_NAME")
                .map_err(|error| DbError::driver("snapshot physical column name", error))?;
            let kind: String = column
                .try_get("COLUMN_TYPE")
                .map_err(|error| DbError::driver("snapshot physical column type", error))?;
            let charset: Option<String> = column
                .try_get("CHARACTER_SET_NAME")
                .map_err(|error| DbError::driver("snapshot physical charset", error))?;
            if let Some(charset) = charset {
                // 从空标量子查询的已解析物理类型读取字符集，不依赖 RR 字典快照或行是否存在。
                let quoted_column = TableMeta::quote_identifier(&name)?;
                let sql = format!("SELECT CHARSET((SELECT {quoted_column} FROM {quoted} LIMIT 0))");
                let physical_charset: String = sqlx::query_scalar(&sql)
                    .fetch_one(&mut **tx)
                    .await
                    .map_err(|error| DbError::driver("current physical charset", error))?;
                if physical_charset != charset {
                    return Err(DbError::Unsupported {
                        feature: "table definition changed after transaction metadata snapshot",
                    });
                }
            }
            let prefix = format!("{} ", TableMeta::quote_identifier(&name)?);
            let current = definition
                .lines()
                .map(str::trim)
                .find_map(|line| line.strip_prefix(&prefix));
            // 生成属性可位于 CHARACTER SET/COLLATE 之后。它在表达式和注释的字符串之前出现。
            if current
                .and_then(|line| line.split(char::from(39)).next())
                .is_some_and(|attributes| attributes.contains(" GENERATED ALWAYS AS "))
            {
                return Err(DbError::Unsupported {
                    feature: "current physical generated column",
                });
            }
            // AUTO_INCREMENT 位于标准输出的类型/NOT NULL 后，不能只核对整数类型。
            if current
                .and_then(|line| line.strip_prefix(&kind))
                .is_some_and(|rest| rest.starts_with(" NOT NULL AUTO_INCREMENT"))
            {
                return Err(DbError::Unsupported {
                    feature: "current physical auto increment column",
                });
            }
            // 当前支持的类型均无引号内空格。检查类型后的词边界，防止 int 与 int unsigned 混同。
            let compatible = current
                .and_then(|line| line.strip_prefix(&kind))
                .is_some_and(|rest| {
                    rest == ","
                        || rest.is_empty()
                        || rest.starts_with(" NOT NULL")
                        || rest.starts_with(" NULL")
                        || rest.starts_with(" DEFAULT")
                        || rest.starts_with(" CHARACTER SET")
                        || rest.starts_with(" COLLATE")
                        || rest.starts_with(" COMMENT")
                });
            if !compatible {
                return Err(DbError::Unsupported {
                    feature: "table definition changed after transaction metadata snapshot",
                });
            }
        }
        let mut primary: Vec<_> = primary_order.iter().collect();
        primary.sort_by_key(|(_, ordinal)| **ordinal);
        let names = primary
            .iter()
            .map(|(name, _)| TableMeta::quote_identifier(name))
            .collect::<Result<Vec<_>, _>>()?;
        let expected = format!("PRIMARY KEY ({})", names.join(","));
        // MySQL 8 的 SHOW CREATE 来自当前已打开的表定义，避免重用 RR 字典快照。
        let actual = definition
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("PRIMARY KEY "));
        if actual.map(|line| line.trim_end_matches(',').to_ascii_lowercase())
            != Some(expected.to_ascii_lowercase())
        {
            return Err(DbError::Unsupported {
                feature: "table definition changed after transaction metadata snapshot",
            });
        }
        Ok(())
    }

    fn column(row: &MySqlRow, primary: &HashMap<String, u32>) -> Result<ColumnMeta, DbError> {
        let text = |key| {
            row.try_get::<String, _>(key)
                .map_err(|e| DbError::driver("column field", e))
        };
        let name = text("COLUMN_NAME")?;
        let data_type = text("DATA_TYPE")?;
        let full_type = text("COLUMN_TYPE")?;
        if !text("EXTRA")?.is_empty() || !text("GENERATION_EXPRESSION")?.is_empty() {
            return Err(DbError::Unsupported {
                feature: "generated/invisible/auto/default-updated column",
            });
        }
        let unsigned = full_type
            .split_whitespace()
            .any(|value| value.eq_ignore_ascii_case("unsigned"));
        let kind = match data_type.as_str() {
            "datetime" => ColumnType::LocalDateTime,
            "tinyint" | "smallint" | "mediumint" | "int" | "bigint" => {
                if unsigned {
                    ColumnType::UnsignedInteger
                } else {
                    ColumnType::SignedInteger
                }
            }
            "json" | "varchar" | "char" | "tinytext" | "text" | "mediumtext" | "longtext" => {
                ColumnType::Text
            }
            "varbinary" | "binary" | "tinyblob" | "blob" | "mediumblob" | "longblob" => {
                ColumnType::Binary
            }
            "decimal" | "numeric" => ColumnType::Decimal,
            _ => {
                return Err(DbError::Unsupported {
                    feature: "column type",
                })
            }
        };
        if kind == ColumnType::Text && data_type != "json" {
            let charset: Option<String> = row
                .try_get("CHARACTER_SET_NAME")
                .map_err(|e| DbError::driver("column charset", e))?;
            if !matches!(charset.as_deref(), Some("utf8mb4" | "utf8mb3" | "ascii")) {
                return Err(DbError::Unsupported {
                    feature: "column charset",
                });
            }
        }
        let ordinal: u64 = row
            .try_get("ORDINAL_POSITION")
            .map_err(|e| DbError::driver("column ordinal", e))?;
        let length: Option<u64> = row
            .try_get("CHARACTER_OCTET_LENGTH")
            .map_err(|e| DbError::driver("column length", e))?;
        let precision: Option<u64> = row
            .try_get("NUMERIC_PRECISION")
            .map_err(|e| DbError::driver("column precision", e))?;
        let scale: Option<u64> = row
            .try_get("NUMERIC_SCALE")
            .map_err(|e| DbError::driver("column scale", e))?;
        let nullable = text("IS_NULLABLE")?;
        if nullable != "YES" && nullable != "NO" {
            return Err(DbError::Unsupported {
                feature: "unknown nullability",
            });
        }
        let pk = primary.get(&name.to_ascii_lowercase()).copied();
        ColumnMeta::new(
            name,
            kind,
            nullable == "YES",
            Self::u32(ordinal)?,
            pk,
            length,
            precision.map(Self::u32).transpose()?,
            scale.map(Self::u32).transpose()?,
        )
        .and_then(|column| column.with_domain(Self::domain(row, &data_type, unsigned)?))
        .and_then(|column| column.with_mysql_type(Self::native_type(&data_type, &full_type)?))
        .map_err(DbError::from)
    }
    fn native_type(data_type: &str, full_type: &str) -> Result<MySqlType, MetadataError> {
        // 同值域的TEXT/BLOB子类仍需保留身份；TINYINT(1)只保留事实，不在这里丢失整数值。
        Ok(match data_type {
            "datetime" => MySqlType::DateTime,
            "json" => MySqlType::Json,
            "tinyint" => MySqlType::TinyInt {
                display_width_one: full_type
                    .split_whitespace()
                    .next()
                    .is_some_and(|name| name.eq_ignore_ascii_case("tinyint(1)")),
            },
            "smallint" => MySqlType::SmallInt,
            "mediumint" => MySqlType::MediumInt,
            "int" => MySqlType::Int,
            "bigint" => MySqlType::BigInt,
            "decimal" | "numeric" => MySqlType::Decimal,
            "char" => MySqlType::Char,
            "varchar" => MySqlType::VarChar,
            "tinytext" => MySqlType::TinyText,
            "text" => MySqlType::Text,
            "mediumtext" => MySqlType::MediumText,
            "longtext" => MySqlType::LongText,
            "binary" => MySqlType::Binary,
            "varbinary" => MySqlType::VarBinary,
            "tinyblob" => MySqlType::TinyBlob,
            "blob" => MySqlType::Blob,
            "mediumblob" => MySqlType::MediumBlob,
            "longblob" => MySqlType::LongBlob,
            _ => {
                return Err(MetadataError::Invalid {
                    reason: "native MySQL type",
                })
            }
        })
    }
    fn domain(
        row: &MySqlRow,
        data_type: &str,
        unsigned: bool,
    ) -> Result<ColumnDomain, MetadataError> {
        let bits = match data_type {
            "tinyint" => Some(8),
            "smallint" => Some(16),
            "mediumint" => Some(24),
            "int" => Some(32),
            "bigint" => Some(64),
            _ => None,
        };
        if let Some(bits) = bits {
            return Ok(ColumnDomain::Integer { bits, unsigned });
        }
        match data_type {
            "datetime" => {
                let precision = row.try_get::<u64, _>("DATETIME_PRECISION").map_err(|_| {
                    MetadataError::Invalid {
                        reason: "DATETIME fractional precision",
                    }
                })?;
                Ok(ColumnDomain::LocalDateTime {
                    fractional_seconds: u8::try_from(precision).map_err(|_| {
                        MetadataError::Invalid {
                            reason: "DATETIME fractional precision",
                        }
                    })?,
                })
            }
            "json" => Ok(ColumnDomain::Json),
            "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" => {
                let charset = match row
                    .try_get::<Option<String>, _>("CHARACTER_SET_NAME")
                    .ok()
                    .flatten()
                    .as_deref()
                {
                    Some("ascii") => CharacterSet::Ascii,
                    Some("utf8mb3") => CharacterSet::Utf8Mb3,
                    Some("utf8mb4") => CharacterSet::Utf8Mb4,
                    _ => {
                        return Err(MetadataError::Invalid {
                            reason: "physical character set",
                        })
                    }
                };
                let character_limit = if matches!(data_type, "char" | "varchar") {
                    Some(
                        row.try_get::<u64, _>("CHARACTER_MAXIMUM_LENGTH")
                            .map_err(|_| MetadataError::Invalid {
                                reason: "physical character length",
                            })?,
                    )
                } else {
                    None
                };
                Ok(ColumnDomain::Text {
                    charset,
                    character_limit,
                    fixed_width: data_type == "char",
                })
            }
            "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob" => {
                Ok(ColumnDomain::Binary {
                    fixed_width: data_type == "binary",
                })
            }
            "decimal" | "numeric" => Ok(ColumnDomain::Decimal { unsigned }),
            _ => Err(MetadataError::Invalid {
                reason: "physical column type",
            }),
        }
    }
    fn u32(value: u64) -> Result<u32, DbError> {
        value.try_into().map_err(|_| {
            MetadataError::Invalid {
                reason: "metadata dimension overflow",
            }
            .into()
        })
    }
}

#[cfg(test)]
mod timestamp_tests {
    use super::MySqlTableMetaReader;
    use seata_datasource::MySqlType;

    #[test]
    fn timestamp_is_a_distinct_native_type() {
        let native = MySqlTableMetaReader::native_type("timestamp", "timestamp(6)");
        assert!(native.is_ok(), "TIMESTAMP must have a native identity");
        assert_ne!(native.ok(), Some(MySqlType::DateTime));
    }
}
