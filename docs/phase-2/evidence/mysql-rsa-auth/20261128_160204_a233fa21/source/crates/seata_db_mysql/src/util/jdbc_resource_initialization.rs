use super::JdbcDriverLoader;
use seata_sqlparser::{JavaString, MetadataError};

/// 初始化期间的资源字段、驱动探测与登记入口。
/// 对应 Java: BaseDataSourceResource 的初始化字段与 DefaultResourceManager#registerResource。
pub trait JdbcResourceInitialization: JdbcDriverLoader {
    /// 探测驱动类名，供原 Druid 的 MySQL 和 ClickHouse 分支选择可用驱动。
    /// 对应 Java: Druid JdbcUtils#getDriverClassName 内部的类可用性检查。
    fn class_available(&mut self, class_name: &str) -> Result<bool, MetadataError>;

    /// 保存可空的 resource_group_id，返回宿主写入结果。
    /// 对应 Java: BaseDataSourceResource#setResourceGroupId。
    fn set_resource_group_id(
        &mut self,
        resource_group_id: Option<JavaString>,
    ) -> Result<(), MetadataError>;

    /// 保存从原 JDBC URL 派生的 resource_id，返回宿主写入结果。
    /// 对应 Java: BaseDataSourceResource#setResourceId。
    fn set_resource_id(&mut self, resource_id: JavaString) -> Result<(), MetadataError>;

    /// 保存构造完成的 driver，返回宿主写入结果。
    /// 对应 Java: BaseDataSourceResource#setDriver。
    fn set_driver(&mut self, driver: Self::Driver) -> Result<(), MetadataError>;

    /// 保存识别的 db_type，返回宿主写入结果。
    /// 对应 Java: BaseDataSourceResource#setDbType。
    fn set_db_type(&mut self, db_type: JavaString) -> Result<(), MetadataError>;

    /// 登记当前资源并返回结果。调用发生在初始化及清理成功后。
    /// 对应 Java: DefaultResourceManager#registerResource。
    fn register_resource(&mut self) -> Result<(), MetadataError>;

    /// 在包装 SQL 异常时返回数据源的 Java 字符串表示。
    /// 对应 Java: JdbcUtils 初始化错误中的数据源字符串拼接。
    fn describe_data_source(&mut self) -> Result<JavaString, MetadataError>;
}
