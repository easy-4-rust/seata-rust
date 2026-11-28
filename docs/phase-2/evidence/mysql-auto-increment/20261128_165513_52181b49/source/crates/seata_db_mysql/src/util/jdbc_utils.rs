use super::{JdbcConnection, JdbcDriverLoader, JdbcResourceInitialization, JdbcXaConnection};
use seata_sqlparser::{
    DbTypeParser, DbTypeParserRegistration, DruidJdbcDriverResolver, JavaObjectError, JavaString,
    MetadataError, MetadataLocale, SqlParserType,
};
use std::sync::{Arc, OnceLock};

static DB_TYPE_PARSER: OnceLock<&'static dyn DbTypeParser> = OnceLock::new();

/// RM 的 JDBC 数据库类型识别及资源 ID 构造。
/// 对应 Java: org.apache.seata.rm.datasource.util.JdbcUtils。
pub struct JdbcUtils;

impl JdbcUtils {
    /// 写入 group，使用 acquire 获取普通连接并初始化 host，连接成功关闭后再登记。
    /// resolver 选择驱动类名，locale 用于数据库类型折叠；返回初始化、关闭或登记错误。
    /// 对应 Java: JdbcUtils#initDataSourceResource。
    pub fn init_data_source_resource<H: JdbcResourceInitialization, C: JdbcConnection>(
        host: &mut H,
        group: Option<JavaString>,
        resolver: &DruidJdbcDriverResolver,
        locale: &MetadataLocale,
        acquire: impl FnOnce() -> Result<Option<C>, MetadataError>,
    ) -> Result<(), MetadataError> {
        host.set_resource_group_id(group)?;
        acquire()
            .and_then(|connection| Self::init_and_close(host, connection, resolver, locale))
            .map_err(|error| {
                init_exception(host, "can not init DataSourceResource with ", error)
            })?;
        host.register_resource()
    }

    /// 写入 group，使用 acquire 获取 XA 连接及内层连接。按内层、外层顺序关闭后登记。
    /// resolver 和 locale 用于识别驱动及类型。外层关闭失败按 Java finally 语义覆盖先前结果。
    /// 对应 Java: JdbcUtils#initXADataSourceResource。
    pub fn init_xa_data_source_resource<H: JdbcResourceInitialization, X: JdbcXaConnection>(
        host: &mut H,
        group: Option<JavaString>,
        resolver: &DruidJdbcDriverResolver,
        locale: &MetadataLocale,
        acquire: impl FnOnce() -> Result<Option<X>, MetadataError>,
    ) -> Result<(), MetadataError> {
        host.set_resource_group_id(group)?;
        acquire()
            .and_then(|xa| {
                let mut xa = xa.ok_or_else(|| null_pointer(
                    r#"Cannot invoke "javax.sql.XAConnection.getConnection()" because "xaConnection" is null"#,
                ))?;
                let initialized = xa.get_connection().and_then(|connection| {
                    Self::init_and_close(host, connection, resolver, locale)
                });
                xa.close()?;
                initialized
            })
            .map_err(|error| {
                init_exception(host, "can not get XAConnection from DataSourceResource with ", error)
            })?;
        host.register_resource()
    }

    fn init_and_close<H: JdbcResourceInitialization, C: JdbcConnection>(
        host: &mut H,
        connection: Option<C>,
        resolver: &DruidJdbcDriverResolver,
        locale: &MetadataLocale,
    ) -> Result<(), MetadataError> {
        let mut connection = connection.ok_or_else(|| {
            null_pointer(
                r#"Cannot invoke "java.sql.Connection.getMetaData()" because "connection" is null"#,
            )
        })?;
        let initialized = (|| {
            let url = connection.jdbc_url()?.ok_or_else(|| null_pointer(
                r#"Cannot invoke "String.contains(java.lang.CharSequence)" because "jdbcUrl" is null"#,
            ))?;
            host.set_resource_id(Self::build_resource_id(Some(&url))?)?;
            let driver_name = resolver
                .try_get_driver_class_name(Some(&url), |name| host.class_available(name))?;
            let driver = Self::load_driver(driver_name.as_ref(), host)?;
            host.set_driver(driver)?;
            host.set_db_type(Self::get_db_type(Some(&url), locale)?)
        })();
        let closed = connection.close();
        match (initialized, closed) {
            (Ok(()), result) | (result, Ok(())) => result,
            (Err(error), Err(close_error)) => attach_suppressed(error, close_error),
        }
    }

    /// 用 loader 按上下文、默认加载域的顺序解析 driver_class_name，返回新构造的驱动。
    /// 对应 Java: JdbcUtils#loadDriver。上下文的类缺失允许回退，默认类缺失和指定构造异常包装为 SQLException。
    pub fn load_driver<L: JdbcDriverLoader>(
        driver_class_name: Option<&JavaString>,
        loader: &mut L,
    ) -> Result<L::Driver, MetadataError> {
        let context = match loader.load_context_class(driver_class_name) {
            Err(error) if is_class_not_found(&error) => None,
            other => other?,
        };
        let class = match context {
            Some(class) => class,
            None => loader
                .load_default_class(driver_class_name)
                .map_err(|error| {
                    if is_class_not_found(&error) {
                        sql_exception(error)
                    } else {
                        error
                    }
                })?,
        };
        loader.instantiate_driver(class).map_err(|error| {
            if error.object_exception().is_some_and(|cause| {
                cause.is_instance_of("java.lang.IllegalAccessException")
                    || cause.is_instance_of("java.lang.InstantiationException")
            }) {
                sql_exception(error)
            } else {
                error
            }
        })
    }

    /// 无参数；返回 RM 选定的 Druid 服务单例。发现失败时返回服务错误，成功后缓存实例。
    /// 对应 Java: JdbcUtils#getDbTypeParser。Rust 服务域由已链接的注册项组成。
    /// RM 以 ROOT 语言解析服务名，URL 的数据库类型由 get_db_type 的 locale 决定。
    pub fn get_db_type_parser() -> Result<&'static dyn DbTypeParser, MetadataError> {
        if let Some(parser) = DB_TYPE_PARSER.get() {
            return Ok(*parser);
        }
        let name = SqlParserType::SQL_PARSER_TYPE_DRUID.into();
        let parser = DbTypeParserRegistration::discover(Some(&name), &MetadataLocale::new(""))?;
        Ok(*DB_TYPE_PARSER.get_or_init(|| parser))
    }

    /// 以 jdbc_url 的第一个问号为界构造资源 ID，保留问号之前的全部 UTF-16 码元。
    /// 对应 Java: JdbcUtils#buildResourceId。没有问号时直接返回同一个字符串引用。
    pub fn build_resource_id(jdbc_url: Option<&JavaString>) -> Result<JavaString, MetadataError> {
        let url = jdbc_url.ok_or(MetadataError::NullReference("JdbcUtils.jdbcUrl"))?;
        let units = url.as_utf16();
        if let Some(index) = units.iter().position(|&unit| unit == u16::from(b'?')) {
            Ok(JavaString::from_utf16(units[..index].to_vec()))
        } else {
            Ok(url.clone())
        }
    }

    /// 识别可空 jdbc_url，并按 locale 转小写。未识别时返回原空引用异常类别。
    /// 对应 Java: JdbcUtils#getDbType。先获取 Druid 服务，再解析 URL。
    pub fn get_db_type(
        jdbc_url: Option<&JavaString>,
        locale: &MetadataLocale,
    ) -> Result<JavaString, MetadataError> {
        let db_type = Self::get_db_type_parser()?
            .parse_from_jdbc_url(jdbc_url)?
            .ok_or(MetadataError::NullReference("JdbcUtils.dbType"))?;
        locale.lower(&db_type)
    }
}

fn is_class_not_found(error: &MetadataError) -> bool {
    error
        .object_exception()
        .is_some_and(|cause| cause.is_instance_of("java.lang.ClassNotFoundException"))
}

fn null_pointer(message: &str) -> MetadataError {
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::new("java.lang.NullPointerException", Some(message.into()))
            .with_superclasses([
                "java.lang.RuntimeException",
                "java.lang.Exception",
                "java.lang.Throwable",
            ]),
    ))
}

fn attach_suppressed(error: MetadataError, close: MetadataError) -> Result<(), MetadataError> {
    if let (Some(primary), Some(suppressed)) = (java_exception(&error), java_exception(&close)) {
        match primary.add_suppressed(suppressed) {
            Ok(()) => return Err(MetadataError::ObjectException(primary)),
            Err(MetadataError::InvalidCompatibilityData(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Err(MetadataError::CleanupFailed {
        primary: Box::new(error),
        cleanup: Box::new(close),
    })
}

fn java_exception(error: &MetadataError) -> Option<Arc<JavaObjectError>> {
    if matches!(error, MetadataError::CleanupFailed { .. }) {
        return None;
    }
    if let Some(original) = error.object_exception() {
        return Some(original.clone());
    }
    let class = error.java_exception_class()?;
    const RUNTIME: &[&str] = &[
        "java.lang.RuntimeException",
        "java.lang.Exception",
        "java.lang.Throwable",
    ];
    const ARRAY_INDEX: &[&str] = &[
        "java.lang.IndexOutOfBoundsException",
        "java.lang.RuntimeException",
        "java.lang.Exception",
        "java.lang.Throwable",
    ];
    const CAPACITY: &[&str] = &[
        "java.lang.VirtualMachineError",
        "java.lang.Error",
        "java.lang.Throwable",
    ];
    let ancestors = match error {
        MetadataError::ListIndexOutOfBounds { array: true, .. } => ARRAY_INDEX,
        MetadataError::CollectionCapacity => CAPACITY,
        _ => RUNTIME,
    };
    Some(Arc::new(
        JavaObjectError::new(class, error.java_message().cloned())
            .with_superclasses(ancestors.iter().copied()),
    ))
}

fn init_exception(
    host: &mut impl JdbcResourceInitialization,
    prefix: &str,
    error: MetadataError,
) -> MetadataError {
    if let MetadataError::CleanupFailed { primary, cleanup } = error {
        return MetadataError::CleanupFailed {
            primary: Box::new(init_exception(host, prefix, *primary)),
            cleanup,
        };
    }
    let Some(cause) = error
        .object_exception()
        .filter(|e| e.is_instance_of("java.sql.SQLException"))
    else {
        return error;
    };
    // Java 只在 catch(SQLException) 路径求值数据源描述。
    let description = match host.describe_data_source() {
        Ok(description) => description,
        Err(error) => return error,
    };
    let mut message: Vec<u16> = prefix.encode_utf16().collect();
    message.extend_from_slice(description.as_utf16());
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::with_cause(
            "java.lang.IllegalStateException",
            Some(JavaString::from_utf16(message)),
            Some(cause.clone()),
        )
        .with_superclasses([
            "java.lang.RuntimeException",
            "java.lang.Exception",
            "java.lang.Throwable",
        ]),
    ))
}

fn sql_exception(error: MetadataError) -> MetadataError {
    let MetadataError::ObjectException(cause) = error else {
        return error;
    };
    let message = cause.message_java().cloned();
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::with_cause("java.sql.SQLException", message, Some(cause))
            .with_superclasses(["java.lang.Exception", "java.lang.Throwable"]),
    ))
}
