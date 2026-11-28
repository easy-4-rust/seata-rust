use crate::dialect_escape_rules::{delegate_escape_handler, CaseRule, DialectEscapeRules};
use seata_sqlparser::{
    register_escape_handler, EscapeHandler, EscapeHandlerRegistration, EscapeHandlerScope,
    JdbcConstants, MetadataLocale, DEFAULT_ESCAPE_SYMBOL,
};
use std::{
    collections::HashSet,
    sync::{Arc, OnceLock},
};

static KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

register_escape_handler!(EscapeHandlerRegistration::new(
    JdbcConstants::OSCAR,
    0,
    EscapeHandlerScope::Singleton,
    register,
));

fn register(locale: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    Arc::new(OscarEscapeHandler::new(locale))
}

/// OscarEscapeHandler 的数据库标识符转义处理器。
/// 对应 Java: org.apache.seata.rm.datasource.sql.handler.oscar.OscarEscapeHandler。
pub struct OscarEscapeHandler {
    inner: DialectEscapeRules,
}

impl OscarEscapeHandler {
    /// 接收 locale 建立方言处理器，供关键字大小写转换使用。
    /// 对应 Java: OscarEscapeHandler 构造器。
    pub fn new(locale: MetadataLocale) -> Self {
        Self {
            inner: DialectEscapeRules::new(
                locale,
                &KEYWORDS,
                include_str!("oscar_keywords.txt"),
                CaseRule::UppercaseUnlessTablePresent,
                DEFAULT_ESCAPE_SYMBOL,
            ),
        }
    }
}

delegate_escape_handler!(OscarEscapeHandler);
