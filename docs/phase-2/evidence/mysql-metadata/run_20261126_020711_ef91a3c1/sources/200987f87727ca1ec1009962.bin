use crate::{JavaString, MetadataError};

/// 将可空 JDBC URL 识别为数据库类型名称。
/// 对应 Java: org.apache.seata.sqlparser.util.DbTypeParser。
pub trait DbTypeParser: Send + Sync {
    /// 接收 jdbc_url，返回可空类型名或解析器错误。输入与输出保留 UTF-16。
    /// 对应 Java: DbTypeParser#parseFromJdbcUrl。
    fn parse_from_jdbc_url(
        &self,
        jdbc_url: Option<&JavaString>,
    ) -> Result<Option<JavaString>, MetadataError>;
}
