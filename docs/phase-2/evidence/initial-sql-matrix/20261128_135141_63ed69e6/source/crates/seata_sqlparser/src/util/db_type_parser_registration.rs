use crate::{DbTypeParser, JavaObjectError, JavaString, JdkCasing, MetadataError, MetadataLocale};
use std::sync::Arc;

/// 已链接到程序的 JDBC 类型解析器单例及其服务名、优先级。
/// 对应 Java: DbTypeParser 的 META-INF/services 声明与 @LoadLevel 单例定义。
pub struct DbTypeParserRegistration {
    name: &'static str,
    order: i32,
    parser: &'static dyn DbTypeParser,
}

inventory::collect!(DbTypeParserRegistration);

impl DbTypeParserRegistration {
    /// 注册原服务名 name、有符号优先级 order 和静态实例 parser。
    /// 对应 Java: LoadLevel 的 name、order 和默认单例作用域。
    pub const fn new(name: &'static str, order: i32, parser: &'static dyn DbTypeParser) -> Self {
        Self {
            name,
            order,
            parser,
        }
    }

    /// 按 locale 转小写后查找可空 name，返回最高优先级的已链接单例。
    /// 对应 Java: EnhancedServiceLoader#load 的命名选择。链接注册顺序无保证，最高优先级并列时报错。
    pub fn discover(
        name: Option<&JavaString>,
        locale: &MetadataLocale,
    ) -> Result<&'static dyn DbTypeParser, MetadataError> {
        let name = name.filter(|name| !name.as_utf16().is_empty()).ok_or_else(|| {
            service_error(
                "java.lang.IllegalArgumentException",
                "the name of service provider for [org.apache.seata.sqlparser.util.DbTypeParser] name is null",
            )
        })?;
        let language = locale.language()?;
        let name = JdkCasing::lower_case(name, &language)?;
        let mut selected: Option<&Self> = None;
        let mut tied = false;
        for candidate in inventory::iter::<Self> {
            if JdkCasing::lower_case(&candidate.name.into(), &language)? != name {
                continue;
            }
            match selected {
                None => selected = Some(candidate),
                Some(current) if candidate.order > current.order => {
                    selected = Some(candidate);
                    tied = false;
                }
                Some(current) if candidate.order == current.order => tied = true,
                Some(_) => {}
            }
        }
        if tied {
            return Err(MetadataError::InvalidCompatibilityData(
                "multiple DbTypeParser providers share the highest order",
            ));
        }
        selected
            .map(|candidate| candidate.parser)
            .ok_or_else(missing_provider)
    }
}

fn missing_provider() -> MetadataError {
    service_error(
        "org.apache.seata.common.loader.EnhancedServiceNotFoundException",
        "not found service provider for : org.apache.seata.sqlparser.util.DbTypeParser",
    )
}

fn service_error(class: &str, message: &str) -> MetadataError {
    MetadataError::ObjectException(Arc::new(
        JavaObjectError::new(class, Some(message.into())).with_superclasses([
            "java.lang.RuntimeException",
            "java.lang.Exception",
            "java.lang.Throwable",
        ]),
    ))
}
