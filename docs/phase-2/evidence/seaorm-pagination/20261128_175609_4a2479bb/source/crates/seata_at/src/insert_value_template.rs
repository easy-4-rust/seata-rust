use std::fmt;

/// 工厂验证后的INSERT单元标量模板及原绑定区间；不可由外部构造任意SQL。
/// 对应 Java: org.apache.seata.sqlparser.druid.mysql.MySQLInsertRecognizer#getInsertRows；Rust位置模板扩展。
#[derive(Clone, PartialEq, Eq)]
pub struct InsertValueTemplate {
    sql: String,
    parameter_start: usize,
    parameter_count: usize,
}
impl InsertValueTemplate {
    /// 参数为已经通过表达式矩阵的SQL和原绑定区间；返回不可变模板，仅供识别器构造。
    pub(crate) fn from_validated_ast(
        sql: String,
        parameter_start: usize,
        parameter_count: usize,
    ) -> Self {
        Self {
            sql,
            parameter_start,
            parameter_count,
        }
    }
    /// 无参数；返回已验证标量SQL，可能含业务字面量，调用方不得记录到日志。
    pub fn sql(&self) -> &str {
        &self.sql
    }
    /// 无参数；返回本单元第一个参数在完整原绑定序列中的下标。
    pub const fn parameter_start(&self) -> usize {
        self.parameter_start
    }
    /// 无参数；返回本单元真实占位符数量，字面量问号不计入。
    pub const fn parameter_count(&self) -> usize {
        self.parameter_count
    }
}
impl fmt::Debug for InsertValueTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InsertValueTemplate")
            .field("sql_bytes", &self.sql.len())
            .field("parameter_start", &self.parameter_start)
            .field("parameter_count", &self.parameter_count)
            .finish()
    }
}
