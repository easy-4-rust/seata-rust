use super::base_mysql_recognizer::BaseMySqlRecognizer;
use crate::{SqlError, SqlRecognizer, SqlType};
use sqlparser::ast::{AssignmentTarget, Statement};

/// 单表UPDATE保留赋值顺序，并分离WHERE参数起点，避免镜像查询错误绑定SET参数。
/// 对应 Java: org.apache.seata.sqlparser.druid.mysql.MySQLUpdateRecognizer。
pub(crate) struct MySqlUpdateRecognizer;
impl MySqlUpdateRecognizer {
    /// 参数为UPDATE AST；返回原表/赋值列/WHERE与绑定起点，拒绝不支持的扩展形状。
    pub(crate) fn recognize(value: &Statement) -> Result<SqlRecognizer, SqlError> {
        let Statement::Update {
            table,
            assignments,
            from: None,
            selection,
            returning: None,
        } = value
        else {
            return Err(SqlError::Unsupported {
                feature: "UPDATE FROM/RETURNING",
            });
        };
        let table_name = BaseMySqlRecognizer::table(table)?;
        let mut names = Vec::new();
        let mut parameter_count = 0;
        for assignment in assignments {
            let AssignmentTarget::ColumnName(name) = &assignment.target else {
                return Err(SqlError::Unsupported {
                    feature: "tuple assignment",
                });
            };
            BaseMySqlRecognizer::object(name)?;
            names.push(name.0[0].clone());
            BaseMySqlRecognizer::expression(&assignment.value, &mut parameter_count, 0, true)?;
        }
        let columns = BaseMySqlRecognizer::columns(&names)?;
        let predicate_parameter_start = 0;
        let predicate_sql = BaseMySqlRecognizer::predicate(selection, &mut parameter_count)?;
        Ok(SqlRecognizer {
            sql: String::new(),
            sql_type: SqlType::Update,
            table_name,
            columns,
            parameter_count,
            predicate_parameter_start,
            predicate_sql,
            insert_row_count: 0,
        })
    }
}
