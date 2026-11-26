/// 明确选择已现场对照的Connector8字段配置；不代表所有JDBC驱动版本。
/// 对应 Java: com.mysql.cj.jdbc.DatabaseMetaData的tinyInt1isBit配置；Rust显式转换策略扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MySqlJdbcProfile {
    /// 原8.0.27默认配置；signed TINYINT(1)只对物理0/1允许无损Boolean编码。
    Connector8Default,
    /// 原8.0.27 tinyInt1isBit=false，完整保留整数值。
    Connector8NumericTinyInt,
}
