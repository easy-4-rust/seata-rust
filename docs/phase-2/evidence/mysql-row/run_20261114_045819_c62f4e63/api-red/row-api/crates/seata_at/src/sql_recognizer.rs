use crate::SqlType;
use std::fmt;

/// 完整SQL验证后的不可变识别结果；不是数据库已执行或表元数据已安全的证明。
/// 对应 Java: org.apache.seata.sqlparser.SQLRecognizer的类型/表/原SQL/列/条件信息。
#[derive(Clone, PartialEq, Eq)]
pub struct SqlRecognizer {
    pub(crate) sql: String,
    pub(crate) sql_type: SqlType,
    pub(crate) table_name: String,
    pub(crate) columns: Vec<String>,
    pub(crate) parameter_count: usize,
    pub(crate) predicate_parameter_start: usize,
    pub(crate) predicate_sql: Option<String>,
    pub(crate) insert_row_count: usize,
}
impl SqlRecognizer {
    /// 无参数；返回原SQL借用；调用方不得将其直接写入日志。
    pub fn sql(&self) -> &str {
        &self.sql
    }
    /// 无参数；返回明确SQL类别。
    pub const fn sql_type(&self) -> SqlType {
        self.sql_type
    }
    /// 无参数；返回解引用引号后的原大小写单表名称。
    pub fn table_name(&self) -> &str {
        &self.table_name
    }
    /// 无参数；返回INSERT列或UPDATE赋值列，保留原顺序；DELETE返回空切片。
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
    /// 无参数；返回真正占位符总数，不计入引号文本里的问号。
    pub const fn parameter_count(&self) -> usize {
        self.parameter_count
    }
    /// 无参数；返回WHERE参数在原绑定序列中的起始下标，便于镜像查询复用正确切片。
    pub const fn predicate_parameter_start(&self) -> usize {
        self.predicate_parameter_start
    }
    /// 无参数；返回AST生成的WHERE表达式（不含WHERE）；缺省不被改成虚构条件。
    pub fn predicate_sql(&self) -> Option<&str> {
        self.predicate_sql.as_deref()
    }
    /// 无参数；返回VALUES行数，非INSERT返回0；不是数据库实际影响行数。
    pub const fn insert_row_count(&self) -> usize {
        self.insert_row_count
    }
}
impl fmt::Debug for SqlRecognizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqlRecognizer")
            .field("sql_type", &self.sql_type)
            .field("sql_bytes", &self.sql.len())
            .field("column_count", &self.columns.len())
            .field("parameter_count", &self.parameter_count)
            .field("insert_row_count", &self.insert_row_count)
            .finish()
    }
}
