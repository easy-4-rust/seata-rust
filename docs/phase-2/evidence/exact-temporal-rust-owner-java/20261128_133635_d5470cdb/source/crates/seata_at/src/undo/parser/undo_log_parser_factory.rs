use super::{UndoLogParser, UndoLogParserSelectionError};

/// Undo 解析器工厂，应用已移除格式规则及旧名称别名后使用已登记单例。
/// 对应 Java: org.apache.seata.rm.datasource.undo.UndoLogParserFactory。
pub struct UndoLogParserFactory;

impl UndoLogParserFactory {
    /// 无参数；返回使用固定默认配置的 Jackson 解析器。
    /// 对应 Java: UndoLogParserFactory#getInstance() 在未覆盖序列化配置时的语义。
    pub fn get_instance() -> Result<&'static UndoLogParser, UndoLogParserSelectionError> {
        Self::get_instance_by_name(Some("jackson"))
    }

    /// 参数为可空序列化器名称，返回选中的单例，或明确的名称及查找错误。
    /// 对应 Java: UndoLogParserFactory#getInstance(String)。
    pub fn get_instance_by_name(
        name: Option<&str>,
    ) -> Result<&'static UndoLogParser, UndoLogParserSelectionError> {
        let name = name.ok_or(UndoLogParserSelectionError::NullFactoryName)?;
        // Java equalsIgnoreCase 将长 s（ſ）也视为 s；该规则仅用于拒绝 FST。
        let mut letters = name.chars();
        if matches!(
            (
                letters.next(),
                letters.next(),
                letters.next(),
                letters.next()
            ),
            (
                Some('f' | 'F'),
                Some('s' | 'S' | 'ſ'),
                Some('t' | 'T'),
                None
            )
        ) {
            return Err(UndoLogParserSelectionError::RemovedFst);
        }
        let resolved = if name.eq_ignore_ascii_case("fury") {
            "fory"
        } else {
            name
        };
        UndoLogParser::load(Some(resolved))
    }
}
