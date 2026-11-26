use super::UndoLogDocument;
use std::fmt;

/// 已验证原分支身份和物理读取范围的 Fastjson Undo 文档。
/// 对应 Java: FastjsonUndoLogParser#decode 与 AbstractUndoLogManager#getTableMeta 的边界。
pub struct FastjsonUndoDocument {
    pub(crate) document: UndoLogDocument,
    pub(crate) tables: Vec<String>,
    pub(crate) encoded_bytes: usize,
    pub(crate) normalized_bytes: usize,
}

impl FastjsonUndoDocument {
    /// 无参数；按日志中首次出现顺序返回去重后的表名。
    pub fn table_names(&self) -> impl ExactSizeIterator<Item = &str> {
        self.tables.iter().map(String::as_str)
    }
}

impl fmt::Debug for FastjsonUndoDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FastjsonUndoDocument")
            .field("table_count", &self.tables.len())
            .field("encoded_bytes", &self.encoded_bytes)
            .field("normalized_bytes", &self.normalized_bytes)
            .finish_non_exhaustive()
    }
}
