use super::java_primary_key_order::JavaPrimaryKeyOrder;
use super::lock_key_output::LockKeyOutput;
use crate::{LockKeyError, MySqlJdbcProfile};
use seata_datasource::{ColumnDomain, DecimalValue, Field, FieldValue, MySqlType, TableRecords};

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
        if rows_including_pk.rows().is_empty() {
            return Ok(None);
        }
        let mut output = LockKeyOutput::new();
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
            let fields: Vec<_> = row.primary_keys().collect();
            for (pk_index, index) in primary_order.iter().enumerate() {
                if pk_index != 0 {
                    output.push("_")?;
                }
                Self::field(&mut output, fields[*index], profile)?;
            }
        }
        Ok(Some(output.finish()))
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
            && profile == MySqlJdbcProfile::Connector8Default
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
