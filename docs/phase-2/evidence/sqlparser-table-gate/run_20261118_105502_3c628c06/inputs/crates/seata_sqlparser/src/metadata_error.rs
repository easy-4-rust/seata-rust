use crate::{JavaObjectError, JavaString};
use std::{error::Error, fmt, sync::Arc};

/// Rust 扩展：元数据共享访问的明确失败，不把锁中毒伪装成空字段。
/// 对应 Java: 元数据对象访问与空引用异常的语言边界；不计入31对象分母。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataError {
    /// 指定对象的共享锁中毒，无法提供可信状态。
    LockPoisoned(&'static str),
    /// 原方法对指定可空字段解引用，映射 Java NullPointerException。
    NullReference(&'static str),
    /// 集合迭代期间检测到结构修改，保留Java的fail-fast异常类别。
    ConcurrentModification,
    /// 固定长度列表不支持指定结构操作。
    UnsupportedListOperation(&'static str),
    /// 不可变集合不支持指定修改操作，保留原JDK异常类别。
    UnsupportedCollectionOperation(&'static str),
    /// 列表下标越界；array=true表示Arrays.asList的数组访问异常。
    ListIndexOutOfBounds {
        /// 调用方提供的原始有符号下标。
        index: i32,
        /// 检查时的逻辑元素数量。
        size: usize,
        /// 是否保留固定数组列表的数组越界异常类别。
        array: bool,
    },
    /// 原Java集合无法表达更大的长度。
    CollectionCapacity,
    /// 固定兼容性数据格式或内部状态无效；Rust扩展，不伪造Java业务异常。
    InvalidCompatibilityData(&'static str),
    /// 原TableMeta不支持缺少主键的表，消息保留原UTF16。
    NotSupportYet(JavaString),
    /// 用户动态对象方法抛出的原异常，Arc保留原身份而不是复制成普通失败。
    ObjectException(Arc<JavaObjectError>),
}

impl MetadataError {
    /// 返回对应Java异常类名；无参数，Rust专属锁错误返回None。
    /// 对应 Java: 元数据及JDK集合的异常类别，不伪造JVM生成的隐式异常消息。
    pub fn java_exception_class(&self) -> Option<&str> {
        match self {
            Self::LockPoisoned(_) => None,
            Self::NullReference(_) => Some("java.lang.NullPointerException"),
            Self::ConcurrentModification => Some("java.util.ConcurrentModificationException"),
            Self::UnsupportedListOperation(_) => Some("java.lang.UnsupportedOperationException"),
            Self::UnsupportedCollectionOperation(_) => {
                Some("java.lang.UnsupportedOperationException")
            }
            Self::ListIndexOutOfBounds { array: true, .. } => {
                Some("java.lang.ArrayIndexOutOfBoundsException")
            }
            Self::ListIndexOutOfBounds { array: false, .. } => {
                Some("java.lang.IndexOutOfBoundsException")
            }
            Self::CollectionCapacity => Some("java.lang.OutOfMemoryError"),
            Self::InvalidCompatibilityData(_) => None,
            Self::NotSupportYet(_) => {
                Some("org.apache.seata.common.exception.NotSupportYetException")
            }
            Self::ObjectException(error) => Some(error.class_name()),
        }
    }

    /// 返回动态回调原异常的共享引用，无参数；其他错误类别返回None。
    /// 对应 Java: 捕获回调抛出的同一Throwable引用
    pub fn object_exception(&self) -> Option<&Arc<JavaObjectError>> {
        match self {
            Self::ObjectException(error) => Some(error),
            _ => None,
        }
    }

    /// 返回显式Java异常原消息；无参数，隐式JVM异常的实现相关消息不伪造。
    /// 对应 Java: Throwable#getMessage的元数据失败边界。
    pub fn java_message(&self) -> Option<&JavaString> {
        match self {
            Self::NotSupportYet(message) => Some(message),
            Self::ObjectException(error) => error.message_java(),
            _ => None,
        }
    }

    /// 无参数，判断是否落入Map.equals原catch的NPE/CCE家族；自定义异常依赖显式父类信息。
    /// 对应 Java: AbstractMap#equals的catch(NullPointerException|ClassCastException)。
    pub(crate) fn caught_by_map_equals(&self) -> bool {
        match self {
            Self::NullReference(_) => true,
            Self::ObjectException(error) => {
                error.is_instance_of("java.lang.NullPointerException")
                    || error.is_instance_of("java.lang.ClassCastException")
            }
            _ => false,
        }
    }
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned(object) => write!(f, "Metadata lock poisoned: {object}"),
            Self::NullReference(field) => write!(f, "Null metadata reference: {field}"),
            Self::ConcurrentModification => f.write_str("Metadata list concurrently modified"),
            Self::UnsupportedListOperation(operation) => {
                write!(f, "Unsupported metadata list operation: {operation}")
            }
            Self::UnsupportedCollectionOperation(operation) => {
                write!(f, "Unsupported metadata collection operation: {operation}")
            }
            Self::ListIndexOutOfBounds { index, size, .. } => {
                write!(f, "Metadata list index {index}, size {size}")
            }
            Self::CollectionCapacity => {
                f.write_str("Metadata list exceeds Java collection capacity")
            }
            Self::InvalidCompatibilityData(reason) => {
                write!(f, "Invalid metadata compatibility data: {reason}")
            }
            Self::NotSupportYet(message) => fmt::Display::fmt(message, f),
            Self::ObjectException(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl Error for MetadataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ObjectException(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
