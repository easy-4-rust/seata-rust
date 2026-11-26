use crate::{dialect_escape_rules::delegate_escape_handler, MySQLEscapeHandler};
use seata_sqlparser::{
    register_escape_handler, EscapeHandler, EscapeHandlerRegistration, EscapeHandlerScope,
    JdbcConstants, MetadataLocale,
};
use std::sync::Arc;

register_escape_handler!(EscapeHandlerRegistration::new(
    JdbcConstants::POLARDBX,
    0,
    EscapeHandlerScope::Singleton,
    register,
));

fn register(locale: MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync> {
    Arc::new(PolarDBXEscapeHandler::new(locale))
}

/// PolarDBXEscapeHandler 的数据库标识符转义处理器。
/// 对应 Java: org.apache.seata.rm.datasource.sql.handler.polardbx.PolarDBXEscapeHandler。
pub struct PolarDBXEscapeHandler {
    inner: MySQLEscapeHandler,
}

impl PolarDBXEscapeHandler {
    /// 接收 locale 建立方言处理器，供关键字大小写转换使用。
    /// 对应 Java: PolarDBXEscapeHandler 构造器。
    pub fn new(locale: MetadataLocale) -> Self {
        Self {
            inner: MySQLEscapeHandler::new(locale),
        }
    }
}

delegate_escape_handler!(PolarDBXEscapeHandler);
