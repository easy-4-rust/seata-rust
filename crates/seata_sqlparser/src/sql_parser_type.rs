/// SQL 解析器实现的服务名称常量空间；不代表对应实现已经迁移。
/// 对应 Java: org.apache.seata.sqlparser.SqlParserType
#[derive(Debug)]
pub struct SqlParserType {
    _private: (),
}

impl SqlParserType {
    /// Druid 解析器的服务名称。
    pub const SQL_PARSER_TYPE_DRUID: &'static str = "druid";
    /// ANTLR 解析器的服务名称。
    pub const SQL_PARSER_TYPE_ANTLR: &'static str = "antlr";
}
