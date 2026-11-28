use crate::dialect_escape_rules::{delegate_escape_handler, CaseRule, DialectEscapeRules};
use seata_sqlparser::{
    register_escape_handler, EscapeHandler, EscapeHandlerRegistration, EscapeHandlerScope,
    EscapeSymbol, JdbcConstants, MetadataLocale,
};
use std::{
    collections::HashSet,
    sync::{Arc, OnceLock},
};

static KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

register_escape_handler!(EscapeHandlerRegistration::new(
    JdbcConstants::SQLSERVER,
    0,
    EscapeHandlerScope::Singleton,
    register,
));

fn register(locale: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    Arc::new(SqlServerEscapeHandler::new(locale))
}

/// SqlServerEscapeHandler 的数据库标识符转义处理器。
/// 对应 Java: org.apache.seata.rm.datasource.sql.handler.sqlserver.SqlServerEscapeHandler。
pub struct SqlServerEscapeHandler {
    inner: DialectEscapeRules,
}

impl SqlServerEscapeHandler {
    /// 接收 locale 建立方言处理器，供关键字大小写转换使用。
    /// 对应 Java: SqlServerEscapeHandler 构造器。
    pub fn new(locale: MetadataLocale) -> Self {
        Self {
            inner: DialectEscapeRules::new(
                locale,
                &KEYWORDS,
                include_str!("sqlserver_keywords.txt"),
                CaseRule::KeywordsOnly,
                EscapeSymbol::paired(u16::from(b'['), u16::from(b']')),
            ),
        }
    }
}

delegate_escape_handler!(SqlServerEscapeHandler);
