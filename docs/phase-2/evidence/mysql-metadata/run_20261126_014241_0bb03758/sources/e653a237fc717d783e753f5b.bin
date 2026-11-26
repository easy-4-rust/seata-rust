use super::{
    undo_log_parser_registration::UndoLogParserRegistration, Fastjson2UndoLogParser,
    FastjsonUndoLogParser, ForyUndoLogParser, JacksonUndoLogParser, KryoUndoLogParser,
    ProtostuffUndoLogParser, UndoCodecError, UndoLogParserSelectionError,
};

/// 固定 Java Undo SPI 已迁移的本地实现，保留每种解析器的实际类型。
/// 对应 Java: org.apache.seata.rm.datasource.undo.UndoLogParser。
#[derive(Debug)]
pub enum UndoLogParser {
    /// Fastjson 文本格式。
    Fastjson(FastjsonUndoLogParser),
    /// Jackson 文本格式。
    Jackson(JacksonUndoLogParser),
    /// Protostuff 二进制格式。
    Protostuff(ProtostuffUndoLogParser),
    /// Kryo 二进制格式。
    Kryo(KryoUndoLogParser),
    /// Fastjson2 JSONB 格式。
    Fastjson2(Fastjson2UndoLogParser),
    /// Fory 兼容二进制格式。
    Fory(ForyUndoLogParser),
}

impl UndoLogParser {
    /// 参数为可空 SPI 名称，返回进程共享的已登记解析器；名称大小写不敏感。
    /// 对应 Java: EnhancedServiceLoader#load(UndoLogParser.class, name)。
    pub fn load(name: Option<&str>) -> Result<&'static Self, UndoLogParserSelectionError> {
        let name = name
            .filter(|name| !name.is_empty())
            .ok_or(UndoLogParserSelectionError::EmptyName)?;
        inventory::iter::<UndoLogParserRegistration>
            .into_iter()
            .find(|candidate| {
                name.chars()
                    .flat_map(char::to_lowercase)
                    .eq(candidate.name.chars())
            })
            .map(UndoLogParserRegistration::instance)
            .ok_or(UndoLogParserSelectionError::ProviderNotFound)
    }

    /// 无参数；返回该解析器生成的原默认 Undo 字节。
    /// 对应 Java: UndoLogParser#getDefaultContent。
    pub fn default_content(&self) -> Result<Vec<u8>, UndoCodecError> {
        match self {
            Self::Fastjson(codec) => Ok(codec.default_content().to_vec()),
            Self::Jackson(codec) => Ok(codec.default_content().to_vec()),
            Self::Protostuff(codec) => codec.default_content(),
            Self::Kryo(codec) => codec.default_content(),
            Self::Fastjson2(codec) => codec.default_content(),
            Self::Fory(codec) => codec.default_content(),
        }
    }
}
