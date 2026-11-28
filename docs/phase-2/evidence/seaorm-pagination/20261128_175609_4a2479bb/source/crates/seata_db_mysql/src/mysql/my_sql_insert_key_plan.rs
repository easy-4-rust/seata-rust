use crate::{DbError, MySqlStatementResult};
use seata_datasource::{ColumnType, Field, FieldValue, TableMeta};

/// 显式键与单行服务端生成键的执行前计划，不预测批量主键或自增步长。
/// 对应 Java: MySQLInsertExecutor#getPkValuesByColumn 的 Rust 有界键来源扩展。
#[derive(Debug)]
pub(crate) enum MySqlInsertKeyPlan {
    Explicit(Vec<Vec<FieldValue>>),
    Generated,
}
impl MySqlInsertKeyPlan {
    /// 参数为实际 VALUES 行数；只允许单行使用服务端首 ID，返回执行前计划。
    pub(crate) fn generated(rows: usize) -> Result<Self, DbError> {
        if rows != 1 {
            return Err(DbError::Unsupported {
                feature: "multiple generated INSERT primary keys",
            });
        }
        Ok(Self::Generated)
    }
    /// 参数为服务器真实列赋值后的键、自增属性及原会话模式；返回执行前来源计划。
    pub(crate) fn assigned(
        keys: Vec<Vec<FieldValue>>,
        auto: bool,
        no_auto_zero: bool,
    ) -> Result<Self, DbError> {
        let generated = auto
            && keys.iter().any(|key| {
                key.first().is_some_and(|v| {
                    matches!(v, FieldValue::Null)
                        || (!no_auto_zero
                            && matches!(v, FieldValue::Signed(0) | FieldValue::Unsigned(0)))
                })
            });
        if generated {
            Self::generated(keys.len())
        } else {
            Ok(Self::Explicit(keys))
        }
    }
    /// 参数为原 DML 回执与已核对元数据；返回镜像用的真实键，整数越界或缺失回执拒绝。
    pub(crate) fn resolve(
        self,
        result: MySqlStatementResult,
        table: &TableMeta,
    ) -> Result<Vec<Vec<FieldValue>>, DbError> {
        match self {
            Self::Explicit(keys) => Ok(keys),
            Self::Generated => {
                if result.rows_affected() != 1 || result.last_insert_id() == 0 {
                    return Err(DbError::Input {
                        reason: "single generated INSERT requires original nonzero ID and one row",
                    });
                }
                let column = table.primary_key_columns().next().ok_or(DbError::Input {
                    reason: "generated primary metadata",
                })?;
                if !column.is_auto_increment() {
                    return Err(DbError::Input {
                        reason: "generated primary is not auto increment",
                    });
                }
                let value = match column.kind() {
                    ColumnType::UnsignedInteger => FieldValue::Unsigned(result.last_insert_id()),
                    ColumnType::SignedInteger => FieldValue::Signed(
                        i64::try_from(result.last_insert_id()).map_err(|_| DbError::Input {
                            reason: "generated signed primary overflow",
                        })?,
                    ),
                    _ => {
                        return Err(DbError::Input {
                            reason: "generated primary integer domain",
                        })
                    }
                };
                Field::new(column, value.clone())?;
                Ok(vec![vec![value]])
            }
        }
    }
}
