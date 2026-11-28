use super::java_primary_key_order::JavaPrimaryKeyOrder;
use super::lock_key_output::LockKeyOutput;
use crate::{LockKeyError, MySqlJdbcProfile};
use seata_datasource::{
    ColumnDomain, ColumnType, DecimalValue, Field, FieldValue, MySqlType, TableRecords,
};

/// 原事务执行器的主键锁键生成路径；不执行SQL、登记分支或声称原对象整体迁移完成。
/// 对应 Java: org.apache.seata.rm.datasource.exec.BaseTransactionalExecutor（部分实现）。
#[derive(Debug)]
pub struct BaseTransactionalExecutor;
impl BaseTransactionalExecutor {
    /// 参数为完整数据库镜像及明确JDBC配置；空镜像返回None，否则按原行/主键顺序返回锁键。
    /// 对应 Java: BaseTransactionalExecutor#buildLockKey；返回格式本身不证明TC锁覆盖安全。
    pub fn build_lock_key(
        rows_including_pk: &TableRecords,
        profile: MySqlJdbcProfile,
    ) -> Result<Option<String>, LockKeyError> {
        Self::encode_lock_key(rows_including_pk, profile, false)
    }

    /// 参数为真实镜像和 JDBC 配置；拒绝 Java TC 无法完整收集的行及含歧义的复合文本主键。
    /// 对应 Java: AbstractLockManager#collectRowLocks 的行解析边界。
    pub(crate) fn registration_lock_key(
        records: &TableRecords,
        profile: MySqlJdbcProfile,
    ) -> Result<Option<String>, LockKeyError> {
        Self::encode_lock_key(records, profile, true)
    }

    fn encode_lock_key(
        rows_including_pk: &TableRecords,
        profile: MySqlJdbcProfile,
        check_row_coverage: bool,
    ) -> Result<Option<String>, LockKeyError> {
        if rows_including_pk.rows().is_empty() {
            return Ok(None);
        }
        let mut output = LockKeyOutput::new();
        let multiple_text_components = check_row_coverage
            && rows_including_pk
                .table_meta()
                .primary_key_columns()
                .filter(|column| column.kind() == ColumnType::Text)
                .nth(1)
                .is_some();
        let primary_names: Vec<_> = rows_including_pk
            .table_meta()
            .primary_key_columns()
            .map(|column| column.name())
            .collect();
        let primary_order = JavaPrimaryKeyOrder::indices(&primary_names)?;
        output.push(rows_including_pk.table_name())?;
        output.push(":")?;
        for (row_index, row) in rows_including_pk.rows().iter().enumerate() {
            if row_index != 0 {
                output.push(",")?;
            }
            let row_start = output.as_str().len();
            let fields: Vec<_> = row.primary_keys().collect();
            for (pk_index, index) in primary_order.iter().enumerate() {
                if pk_index != 0 {
                    output.push("_")?;
                }
                Self::field(&mut output, fields[*index], profile)?;
            }
            if check_row_coverage {
                let token = &output.as_str()[row_start..];
                Self::check_registration_row(token)?;
                // 原格式用下划线连接元组。至少两个文本分量时，值中的下划线可能被解释成边界。
                if multiple_text_components
                    && token.bytes().filter(|byte| *byte == b'_').count() > primary_order.len() - 1
                {
                    return Err(LockKeyError::Unsupported {
                        feature: "ambiguous delimiter in composite primary key",
                    });
                }
            }
        }
        Ok(Some(output.finish()))
    }

    fn check_registration_row(key: &str) -> Result<(), LockKeyError> {
        // 固定 JDK 的 Character.isWhitespace(char)，与 Rust Unicode White_Space 不同。
        let java_blank = key.encode_utf16().all(|unit| {
            matches!(unit, 0x0009..=0x000d | 0x001c..=0x0020 | 0x1680
                | 0x2000..=0x2006 | 0x2008..=0x200a | 0x2028 | 0x2029 | 0x205f | 0x3000)
        });
        if java_blank || key.contains([',', ';']) {
            return Err(LockKeyError::Unsupported {
                feature: "Java TC drops or splits the original row lock token",
            });
        }
        Ok(())
    }
    fn field(
        output: &mut LockKeyOutput,
        field: &Field,
        profile: MySqlJdbcProfile,
    ) -> Result<(), LockKeyError> {
        let native = field
            .column()
            .mysql_type()
            .ok_or(LockKeyError::Unsupported {
                feature: "missing native type identity",
            })?;
        if native
            == (MySqlType::TinyInt {
                display_width_one: true,
            })
            && !matches!(
                field.column().domain(),
                Some(ColumnDomain::Integer { unsigned: true, .. })
            )
            && profile.boolean_tinyint()
        {
            return match field.value() {
                FieldValue::Signed(0) => output.push("false"),
                FieldValue::Signed(1) => output.push("true"),
                _ => Err(LockKeyError::Unsupported {
                    feature: "lossy tinyint Boolean conversion",
                }),
            };
        }
        match field.value() {
            FieldValue::LocalDate(value) => {
                if !profile.exact_date_time() {
                    return Err(LockKeyError::Unsupported {
                        feature: "explicit exact DATE/TIME String profile required",
                    });
                }
                output.push(&value.to_sql_string())
            }
            FieldValue::InternalTime(value) => {
                if !profile.exact_date_time() {
                    return Err(LockKeyError::Unsupported {
                        feature: "explicit exact DATE/TIME String profile required",
                    });
                }
                let Some(ColumnDomain::InternalTime { fractional_seconds }) =
                    field.column().domain()
                else {
                    return Err(LockKeyError::Unsupported {
                        feature: "TIME physical precision",
                    });
                };
                output.push(&value.to_sql_string(*fractional_seconds).map_err(|_| {
                    LockKeyError::Invalid {
                        reason: "TIME physical precision",
                    }
                })?)
            }
            FieldValue::Timestamp(value) => {
                if !profile.utc_temporal() {
                    return Err(LockKeyError::Unsupported {
                        feature: "explicit UTC TIMESTAMP profile required",
                    });
                }
                output.push(&value.to_java_utc_string())
            }
            FieldValue::LocalDateTime(value) => {
                if !profile.utc_temporal() {
                    return Err(LockKeyError::Unsupported {
                        feature: "explicit UTC DATETIME profile required",
                    });
                }
                output.push(&value.to_java_string())
            }
            FieldValue::Signed(value) => output.push(&value.to_string()),
            FieldValue::Unsigned(value) => output.push(&value.to_string()),
            FieldValue::Text(value) => {
                if value.contains(',') {
                    return Err(LockKeyError::Invalid {
                        reason: "primary value contains comma",
                    });
                }
                output.push(value)
            }
            FieldValue::Binary(value) => {
                output.push("[")?;
                for (index, byte) in value.iter().enumerate() {
                    if index != 0 {
                        output.push(", ")?;
                    }
                    // Java byte有符号；绝不能打印Rust Vec或Java对象地址代替原格式。
                    output.push(&(*byte as i8).to_string())?;
                }
                output.push("]")
            }
            FieldValue::Decimal(value) => output.push(&Self::decimal(value)),
            FieldValue::Null => Err(LockKeyError::Invalid {
                reason: "null primary identity",
            }),
        }
    }
    fn decimal(value: &DecimalValue) -> String {
        let text = value.as_str();
        let unsigned = text.strip_prefix('-').unwrap_or(text);
        let digits: String = unsigned
            .chars()
            .filter(|character| *character != '.')
            .collect();
        let significant = digits.trim_start_matches('0');
        let significant = if significant.is_empty() {
            "0"
        } else {
            significant
        };
        let exponent = significant.len() as i64 - 1 - i64::from(value.scale());
        if exponent >= -6 {
            return text.into();
        }
        let mut result = String::with_capacity(72);
        if text.starts_with('-') {
            result.push('-');
        }
        // 数字在DecimalValue构造时已验证为ASCII，切片下标不会拆开UTF-8。
        result.push_str(&significant[..1]);
        if significant.len() > 1 {
            result.push('.');
            result.push_str(&significant[1..]);
        }
        result.push('E');
        result.push_str(&exponent.to_string());
        result
    }
}
