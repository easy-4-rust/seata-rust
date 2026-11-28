use crate::{EscapeHandler, EscapeHandlerScope, MetadataLocale};
use std::sync::Arc;

/// 链接到当前程序中的转义处理器注册项。
/// 对应 Java: META-INF/services 条目与 @LoadLevel 元数据。
pub struct EscapeHandlerRegistration {
    pub(crate) name: &'static str,
    pub(crate) order: i32,
    pub(crate) scope: EscapeHandlerScope,
    pub(crate) constructor: fn(MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync>,
}

impl EscapeHandlerRegistration {
    /// 用 name、order、scope 和 constructor 声明一个方言服务。
    /// 对应 Java: @LoadLevel 与 provider 构造器。
    pub const fn new(
        name: &'static str,
        order: i32,
        scope: EscapeHandlerScope,
        constructor: fn(MetadataLocale) -> Arc<dyn EscapeHandler + Send + Sync>,
    ) -> Self {
        Self {
            name,
            order,
            scope,
            constructor,
        }
    }
}

inventory::collect!(EscapeHandlerRegistration);
