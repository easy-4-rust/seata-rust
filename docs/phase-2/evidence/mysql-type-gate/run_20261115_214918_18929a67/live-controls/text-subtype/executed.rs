use crate::DbError;
use seata_datasource::{
    CharacterSet, ColumnDomain, ColumnMeta, ColumnType, MetadataError, MySqlType, TableMeta,
};
use sqlx::{mysql::MySqlRow, MySql, Row, Transaction};
use std::collections::HashMap;

/// 持有同事务MDL时读取MySQL真实表元数据，不以跨事务缓存替代当前数据库事实。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.cache.MysqlTableMetaCache#fetchSchema/resultSetMetaToSchema（部分实现）。
pub(crate) struct MySqlTableMetaReader;
impl MySqlTableMetaReader {
    /// 参数为原独占事务、schema与表名；返回精确类型/列序/PK序；未知或不可见形状拒绝。
    pub(crate) async fn fetch(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        table: &str,
    ) -> Result<TableMeta, DbError> {
        let quoted = TableMeta::quote_identifier(table)?;
        if table.eq_ignore_ascii_case("undo_log") {
            return Err(DbError::Unsupported {
                feature: "undo table as business resource",
            });
        }
        sqlx::query(&format!("SELECT 1 FROM {quoted} LIMIT 0 FOR UPDATE"))
            .execute(&mut **tx)
            .await
            .map_err(|error| DbError::driver("metadata lock", error))?;
        Self::visibility(tx, database).await?;
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
        let triggers: i64=sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA=? AND EVENT_OBJECT_TABLE=?")
            .bind(database).bind(table).fetch_one(&mut **tx).await.map_err(|e|DbError::driver("trigger metadata",e))?;
        if triggers != 0 {
            return Err(DbError::Unsupported {
                feature: "table trigger",
            });
        }
        // 全局字典覆盖跨schema入向FK，不能依赖应用权限过滤后的KEY_COLUMN_USAGE空行。
        let foreign: i64=sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.INNODB_FOREIGN WHERE FOR_NAME=CONCAT(?,'/',?) OR REF_NAME=CONCAT(?,'/',?)")
            .bind(database).bind(table).bind(database).bind(table).fetch_one(&mut **tx).await.map_err(|e|DbError::driver("foreign-key metadata",e))?;
        if foreign != 0 {
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
        let columns=sqlx::query("SELECT CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4) AS COLUMN_NAME, CAST(DATA_TYPE AS CHAR CHARACTER SET utf8mb4) AS DATA_TYPE, CAST(COLUMN_TYPE AS CHAR CHARACTER SET utf8mb4) AS COLUMN_TYPE, CAST(IS_NULLABLE AS CHAR CHARACTER SET utf8mb4) AS IS_NULLABLE, CAST(ORDINAL_POSITION AS UNSIGNED) AS ORDINAL_POSITION, CAST(CHARACTER_OCTET_LENGTH AS UNSIGNED) AS CHARACTER_OCTET_LENGTH, CAST(CHARACTER_MAXIMUM_LENGTH AS UNSIGNED) AS CHARACTER_MAXIMUM_LENGTH, CAST(NUMERIC_PRECISION AS UNSIGNED) AS NUMERIC_PRECISION, CAST(NUMERIC_SCALE AS UNSIGNED) AS NUMERIC_SCALE, CAST(EXTRA AS CHAR CHARACTER SET utf8mb4) AS EXTRA, CAST(GENERATION_EXPRESSION AS CHAR CHARACTER SET utf8mb4) AS GENERATION_EXPRESSION, CAST(CHARACTER_SET_NAME AS CHAR CHARACTER SET utf8mb4) AS CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY ORDINAL_POSITION LIMIT 129")
            .bind(database).bind(table).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("column metadata",e))?;
        if columns.len() > 128 {
            return Err(MetadataError::Budget {
                limit: "1..128 columns",
            }
            .into());
        }
        let columns = columns
            .iter()
            .map(|row| Self::column(row, &primary_order))
            .collect::<Result<Vec<_>, _>>()?;
        TableMeta::new(table.into(), columns).map_err(DbError::from)
    }
    async fn visibility(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
    ) -> Result<(), DbError> {
        let grants = sqlx::query("SHOW GRANTS")
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| DbError::driver("metadata visibility", e))?;
        let mut trigger = false;
        let mut process = false;
        let local = format!("`{database}`.*");
        for row in grants {
            let grant: String = row
                .try_get(0)
                .map_err(|e| DbError::driver("grant metadata", e))?;
            if grant.starts_with("REVOKE ") {
                return Err(DbError::Unsupported {
                    feature: "partial metadata privilege revocation",
                });
            }
            let Some(text) = grant.strip_prefix("GRANT ") else {
                continue;
            };
            let Some((privileges, rest)) = text.split_once(" ON ") else {
                continue;
            };
            let Some((scope, _)) = rest.split_once(" TO ") else {
                continue;
            };
            let all = privileges.split(',').any(|p| p.trim() == "ALL PRIVILEGES");
            if scope == "*.*" || scope == local {
                trigger |= all || privileges.split(',').any(|p| p.trim() == "TRIGGER");
            }
            if scope == "*.*" {
                process |= all || privileges.split(',').any(|p| p.trim() == "PROCESS");
            }
        }
        if !trigger {
            return Err(DbError::Unsupported {
                feature: "metadata trigger visibility",
            });
        }
        if !process {
            return Err(DbError::Unsupported {
                feature: "metadata foreign-key visibility",
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
            "tinyint" | "smallint" | "mediumint" | "int" | "bigint" => {
                if unsigned {
                    ColumnType::UnsignedInteger
                } else {
                    ColumnType::SignedInteger
                }
            }
            "varchar" | "char" | "tinytext" | "text" | "mediumtext" | "longtext" => {
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
        if kind == ColumnType::Text {
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
            "tinytext" => MySqlType::Text,
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
