use super::base_mysql_recognizer::BaseMySqlRecognizer;
use crate::{SqlError, SqlRecognizer, SqlType};
use sqlparser::ast::{Delete, FromTable};

/// 单表DELETE解析；多表、USING、RETURNING及ORDER/LIMIT全部拒绝。
/// 对应 Java: org.apache.seata.sqlparser.druid.mysql.MySQLDeleteRecognizer。
pub(crate) struct MySqlDeleteRecognizer;
impl MySqlDeleteRecognizer {
    /// 参数为DELETE AST；返回单表及原可选条件识别结果；此方法不读取或删除数据。
    pub(crate) fn recognize(value: &Delete) -> Result<SqlRecognizer, SqlError> {
        if !value.tables.is_empty()
            || value.using.is_some()
            || value.returning.is_some()
            || !value.order_by.is_empty()
            || value.limit.is_some()
        {
            return Err(SqlError::Unsupported {
                feature: "DELETE multi-table/USING/RETURNING/ORDER/LIMIT",
            });
        }
        let FromTable::WithFromKeyword(tables) = &value.from else {
            return Err(SqlError::Malformed);
        };
        if tables.len() != 1 {
            return Err(SqlError::Unsupported {
                feature: "multiple DELETE tables",
            });
        }
        let (table_name, table_alias) = BaseMySqlRecognizer::table(&tables[0])?;
        let qualifier = table_alias.as_deref().unwrap_or(&table_name);
        let mut parameter_count = 0;
        let predicate_sql =
            BaseMySqlRecognizer::predicate(&value.selection, &mut parameter_count, qualifier)?;
        Ok(SqlRecognizer {
            sql: String::new(),
            sql_type: SqlType::Delete,
            table_name,
            table_alias,
            columns: Vec::new(),
            parameter_count,
            predicate_parameter_start: 0,
            predicate_sql,
            insert_row_count: 0,
            insert_rows: Vec::new(),
        })
    }
}
