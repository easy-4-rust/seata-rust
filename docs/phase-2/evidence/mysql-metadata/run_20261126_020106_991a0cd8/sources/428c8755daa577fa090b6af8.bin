use seata_sqlparser::{JavaString, MetadataError};

/// 宿主提供的初始化连接。
/// 对应 Java: java.sql.Connection。
pub trait JdbcConnection {
    /// 从连接元数据返回可空 URL。元数据为空或读取失败时返回原异常。
    /// 对应 Java: Connection#getMetaData 与 DatabaseMetaData#getURL。
    fn jdbc_url(&mut self) -> Result<Option<JavaString>, MetadataError>;

    /// 消耗连接并关闭它，返回宿主关闭异常。
    /// 对应 Java: Connection#close。
    fn close(self) -> Result<(), MetadataError>;
}
