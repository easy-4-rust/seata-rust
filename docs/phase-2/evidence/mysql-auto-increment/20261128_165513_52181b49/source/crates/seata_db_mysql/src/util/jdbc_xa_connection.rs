use super::JdbcConnection;
use seata_sqlparser::MetadataError;

/// 宿主提供的外层 XA 连接，独立于内层 JDBC 连接关闭。
/// 对应 Java: javax.sql.XAConnection。
pub trait JdbcXaConnection {
    /// 外层连接提供的普通 JDBC 连接句柄。
    type Connection: JdbcConnection;

    /// 获取可空的内层连接或返回宿主异常。
    /// 对应 Java: XAConnection#getConnection。
    fn get_connection(&mut self) -> Result<Option<Self::Connection>, MetadataError>;

    /// 消耗外层连接，返回关闭结果。
    /// 对应 Java: XAConnection#close。
    fn close(self) -> Result<(), MetadataError>;
}
