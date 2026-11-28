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
    JdbcConstants::POSTGRESQL,
    0,
    EscapeHandlerScope::Singleton,
    register,
));

fn register(locale: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    Arc::new(PostgresqlEscapeHandler::new(locale))
}

/// PostgresqlEscapeHandler 的数据库标识符转义处理器。
/// 对应 Java: org.apache.seata.rm.datasource.sql.handler.postgresql.PostgresqlEscapeHandler。
pub struct PostgresqlEscapeHandler {
    inner: DialectEscapeRules,
}

impl PostgresqlEscapeHandler {
    /// 接收 locale 建立方言处理器，供关键字大小写转换使用。
    /// 对应 Java: PostgresqlEscapeHandler 构造器。
    pub fn new(locale: MetadataLocale) -> Self {
        Self {
            inner: DialectEscapeRules::new(
                locale,
                &KEYWORDS,
                include_str!("postgresql_keywords.txt"),
                CaseRule::LowercaseUnlessTablePresent,
                DEFAULT_ESCAPE_SYMBOL,
            ),
        }
    }
}

delegate_escape_handler!(PostgresqlEscapeHandler);
