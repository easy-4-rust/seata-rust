use super::UndoLogParser;
use std::sync::OnceLock;

/// 将固定 Java Undo SPI 服务描述映射为 Rust 链接期注册和单例初始化。
/// 对应 Java: META-INF/services/org.apache.seata.rm.datasource.undo.UndoLogParser。
pub(super) struct UndoLogParserRegistration {
    pub(super) name: &'static str,
    get: fn() -> &'static UndoLogParser,
}

impl UndoLogParserRegistration {
    const fn new(name: &'static str, get: fn() -> &'static UndoLogParser) -> Self {
        Self { name, get }
    }

    pub(super) fn instance(&'static self) -> &'static UndoLogParser {
        (self.get)()
    }
}

inventory::collect!(UndoLogParserRegistration);

inventory::submit! {
    UndoLogParserRegistration::new("fastjson", || {
        static INSTANCE: OnceLock<UndoLogParser> = OnceLock::new();
        INSTANCE.get_or_init(|| UndoLogParser::Fastjson(Default::default()))
    })
}
inventory::submit! {
    UndoLogParserRegistration::new("jackson", || {
        static INSTANCE: OnceLock<UndoLogParser> = OnceLock::new();
        INSTANCE.get_or_init(|| UndoLogParser::Jackson(Default::default()))
    })
}
inventory::submit! {
    UndoLogParserRegistration::new("protostuff", || {
        static INSTANCE: OnceLock<UndoLogParser> = OnceLock::new();
        INSTANCE.get_or_init(|| UndoLogParser::Protostuff(Default::default()))
    })
}
inventory::submit! {
    UndoLogParserRegistration::new("kryo", || {
        static INSTANCE: OnceLock<UndoLogParser> = OnceLock::new();
        INSTANCE.get_or_init(|| UndoLogParser::Kryo(Default::default()))
    })
}
inventory::submit! {
    UndoLogParserRegistration::new("fastjson2", || {
        static INSTANCE: OnceLock<UndoLogParser> = OnceLock::new();
        INSTANCE.get_or_init(|| UndoLogParser::Fastjson2(Default::default()))
    })
}
inventory::submit! {
    UndoLogParserRegistration::new("fory", || {
        static INSTANCE: OnceLock<UndoLogParser> = OnceLock::new();
        INSTANCE.get_or_init(|| UndoLogParser::Fory(Default::default()))
    })
}
