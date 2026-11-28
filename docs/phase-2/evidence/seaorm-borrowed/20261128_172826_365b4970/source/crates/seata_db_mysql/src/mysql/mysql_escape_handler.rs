use seata_sqlparser::{
    register_escape_handler, EscapeHandler, EscapeHandlerFactory, EscapeHandlerRegistration,
    EscapeHandlerScope, EscapeSymbol, JavaString, MetadataError, MetadataLocale, TableMeta,
};
use std::{
    collections::HashSet,
    sync::{Arc, OnceLock},
};

static KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

register_escape_handler!(EscapeHandlerRegistration::new(
    "mysql",
    0,
    EscapeHandlerScope::Singleton,
    registered_mysql_handler,
));

fn registered_mysql_handler(locale: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    Arc::new(MySQLEscapeHandler::new(locale))
}

/// MySQL 方言的关键字判断与反引号转义。
/// 对应 Java: org.apache.seata.rm.datasource.sql.handler.mysql.MySQLEscapeHandler。
pub struct MySQLEscapeHandler {
    locale: MetadataLocale,
}

impl MySQLEscapeHandler {
    /// 用 locale 建立处理器，调用时读取共享的大写转换语言设置。
    /// 对应 Java: MySQLEscapeHandler 无参构造及 Locale.getDefault 依赖。
    pub fn new(locale: MetadataLocale) -> Self {
        Self { locale }
    }

    /// 用 locale 构造带 mysql 服务注册的转义工厂。
    /// 对应 Java: MySQLEscapeHandler 的 @LoadLevel(name = JdbcConstants.MYSQL)。
    pub fn factory(locale: MetadataLocale) -> Result<EscapeHandlerFactory, MetadataError> {
        let provider = registered_mysql_handler(locale.clone());
        EscapeHandlerFactory::new(locale, [(JavaString::from("mysql"), provider)])
    }
}

impl EscapeHandler for MySQLEscapeHandler {
    /// 检查可空 name 是否在原关键字集合中，再尝试按当前 Locale 大写。
    /// 对应 Java: MySQLEscapeHandler#checkIfKeyWords。
    fn check_if_key_words(&self, name: Option<&JavaString>) -> Result<bool, MetadataError> {
        let Some(name) = name else {
            return Ok(false);
        };
        let words = KEYWORDS.get_or_init(|| include_str!("mysql_keywords.txt").lines().collect());
        if name.as_str().is_some_and(|s| words.contains(s)) {
            return Ok(true);
        }
        Ok(self
            .locale
            .upper(name)?
            .as_str()
            .is_some_and(|s| words.contains(s)))
    }

    /// 判断可空 name 是否为尚未转义的 MySQL 关键字；原实现不读取 table。
    /// 对应 Java: MySQLEscapeHandler#checkIfNeedEscape。
    fn check_if_need_escape(
        &self,
        name: Option<&JavaString>,
        _table: Option<&TableMeta>,
    ) -> Result<bool, MetadataError> {
        let Some(name) = name.filter(|name| !name.is_java_blank()) else {
            return Ok(false);
        };
        let name = name.java_trim();
        if self.contains_escape(Some(&name))? {
            return Ok(false);
        }
        self.check_if_key_words(Some(&name))
    }

    /// 返回 MySQL 的反引号对。
    /// 对应 Java: MySQLEscapeHandler#getEscapeSymbol。
    fn escape_symbol(&self) -> Result<Option<EscapeSymbol>, MetadataError> {
        Ok(Some(EscapeSymbol::new(u16::from(b'`'))))
    }
}
