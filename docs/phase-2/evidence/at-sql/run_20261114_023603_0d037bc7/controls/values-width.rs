use super::base_mysql_recognizer::BaseMySqlRecognizer;
use crate::{SqlError, SqlRecognizer, SqlType};
use sqlparser::ast::{Insert, SetExpr};

/// INSERT VALUES的原列、各行及参数检查；不接受隐式列、查询插入和冲突改写。
/// 对应 Java: org.apache.seata.sqlparser.druid.mysql.MySQLInsertRecognizer。
pub(crate) struct MySqlInsertRecognizer;
impl MySqlInsertRecognizer {
    /// 参数为INSERT AST；返回有界显式列/VALUES识别结果或明确拒绝，不执行SQL。
    pub(crate) fn recognize(value: &Insert) -> Result<SqlRecognizer, SqlError> {
        if value.or.is_some()
            || value.ignore
            || value.table_alias.is_some()
            || value.overwrite
            || value.partitioned.is_some()
            || !value.after_columns.is_empty()
            || value.table
            || value.on.is_some()
            || value.returning.is_some()
            || value.replace_into
            || value.priority.is_some()
            || value.insert_alias.is_some()
        {
            return Err(SqlError::Unsupported {
                feature: "INSERT modifiers/alias/conflict/returning",
            });
        }
        let table_name = BaseMySqlRecognizer::object(&value.table_name)?;
        let columns = BaseMySqlRecognizer::columns(&value.columns)?;
        let query = value.source.as_ref().ok_or(SqlError::Malformed)?;
        if query.with.is_some()
            || query.order_by.is_some()
            || query.limit.is_some()
            || !query.limit_by.is_empty()
            || query.offset.is_some()
            || query.fetch.is_some()
            || !query.locks.is_empty()
            || query.for_clause.is_some()
            || query.settings.is_some()
            || query.format_clause.is_some()
        {
            return Err(SqlError::Unsupported {
                feature: "INSERT query modifiers",
            });
        }
        let SetExpr::Values(values) = query.body.as_ref() else {
            return Err(SqlError::Unsupported {
                feature: "INSERT SELECT",
            });
        };
        if values.explicit_row {
            return Err(SqlError::Unsupported {
                feature: "VALUES ROW constructor",
            });
        }
        if values.rows.is_empty() || values.rows.len() > 1024 {
            return Err(SqlError::Budget {
                limit: "1..1024 VALUES rows",
            });
        }
        let mut parameter_count = 0;
        for row in &values.rows {
            if false {
                return Err(SqlError::Invalid {
                    reason: "VALUES row width differs from columns",
                });
            }
            for expr in row {
                BaseMySqlRecognizer::expression(expr, &mut parameter_count, 0, false)?;
            }
        }
        Ok(SqlRecognizer {
            sql: String::new(),
            sql_type: SqlType::Insert,
            table_name,
            columns,
            parameter_count,
            predicate_parameter_start: parameter_count,
            predicate_sql: None,
            insert_row_count: values.rows.len(),
        })
    }
}
