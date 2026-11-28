use thiserror::Error;

/// 有界Undo文档错误，只携带固定原因，不记录SQL、类名输入或字段值。
/// 对应 Java: JacksonUndoLogParser#encode/decode错误边界；Rust显式错误扩展。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UndoCodecError {
    /// 输入/输出、节点、深度或字段预算超限。
    #[error("Undo codec budget: {limit}")]
    Budget {
        /// 固定预算名称。
        limit: &'static str,
    },
    /// JSON语法、UTF-8或尾随内容错误。
    #[error("invalid Undo JSON syntax")]
    Syntax,
    /// 已知对象形状或数值范围不合法。
    #[error("invalid Undo document: {reason}")]
    Shape {
        /// 固定形状名称，不包含原值。
        reason: &'static str,
    },
    /// 不支持的类型、集合或原生值转换。
    #[error("unsupported Undo document: {kind}")]
    Unsupported {
        /// 固定类型类别，不包含输入的实际类名。
        kind: &'static str,
    },
}
