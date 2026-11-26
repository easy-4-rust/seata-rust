use std::{error::Error, fmt};

/// Rust 扩展：元数据共享访问的明确失败，不把锁中毒伪装成空字段。
/// 对应 Java: 元数据对象访问与空引用异常的语言边界；不计入31对象分母。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataError {
    /// 指定对象的共享锁中毒，无法提供可信状态。
    LockPoisoned(&'static str),
    /// 原方法对指定可空字段解引用，映射 Java NullPointerException。
    NullReference(&'static str),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned(object) => write!(f, "Metadata lock poisoned: {object}"),
            Self::NullReference(field) => write!(f, "Null metadata reference: {field}"),
        }
    }
}

impl Error for MetadataError {}
