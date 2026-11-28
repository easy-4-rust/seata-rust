use seata_sqlparser::{JavaString, MetadataError};

/// 宿主提供的 JDBC 类查找与驱动构造边界。
/// 对应 Java: ClassLoader#loadClass、Class#forName 及 Class#newInstance。
pub trait JdbcDriverLoader {
    /// 已解析的宿主类句柄。
    type Class;
    /// 构造并验证为 JDBC Driver 的实例。
    type Driver;

    /// 用 driver_class_name 查找线程上下文类。无加载器或得到空类时返回 None。
    /// 对应 Java: Thread#getContextClassLoader 与 ClassLoader#loadClass。
    fn load_context_class(
        &mut self,
        driver_class_name: Option<&JavaString>,
    ) -> Result<Option<Self::Class>, MetadataError>;

    /// 用 driver_class_name 查找并初始化默认加载域的类，返回句柄或宿主异常。
    /// 对应 Java: Class#forName(String)。
    fn load_default_class(
        &mut self,
        driver_class_name: Option<&JavaString>,
    ) -> Result<Self::Class, MetadataError>;

    /// 调用 class 的无参构造器，再验证实例是 JDBC Driver，保留宿主异常的继承关系。
    /// 对应 Java: Class#newInstance 与转换为 java.sql.Driver。
    fn instantiate_driver(&mut self, class: Self::Class) -> Result<Self::Driver, MetadataError>;
}
