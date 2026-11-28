use crate::{JavaString, MetadataError};
use std::{any::Any, fmt::Debug};

/// Rust 扩展：元数据 Object 字段的动态语义桥，允许调用方适配任意真实对象类型。
/// 对应 Java: java.lang.Object 的 equals/hashCode/toString；不冒充完整 Object 移植。
pub trait JavaObject: Debug + Send + Sync + 'static {
    /// 返回动态类型借用，无参数；用于保留 Integer、Long 等不同运行类型。
    fn as_any(&self) -> &dyn Any;
    /// 比较指定other的Java对象语义；不同数值类型不自动相等，回调抛异常时返回原错误。
    fn java_equals(&self, other: &dyn JavaObject) -> Result<bool, MetadataError>;
    /// 返回原对象32位Java hashCode；无参数，回调异常原样返回。
    fn java_hash_code(&self) -> Result<i32, MetadataError>;
    /// 返回可空原Java toString文本；无参数，保留孤立代理码元，回调异常原样返回。
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError>;
}

macro_rules! integral {
    ($type:ty, $hash:expr) => {
        impl JavaObject for $type {
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn java_equals(&self, other: &dyn JavaObject) -> Result<bool, MetadataError> {
                Ok(other.as_any().downcast_ref::<Self>() == Some(self))
            }
            fn java_hash_code(&self) -> Result<i32, MetadataError> {
                Ok(($hash)(*self))
            }
            fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
                Ok(Some(self.to_string().into()))
            }
        }
    };
}

integral!(i8, |value: i8| i32::from(value));
integral!(i16, |value: i16| i32::from(value));
integral!(i32, |value: i32| value);
integral!(i64, |value: i64| (value ^ (value >> 32)) as i32);
integral!(bool, |value: bool| if value { 1231 } else { 1237 });

impl JavaObject for JavaString {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_equals(&self, other: &dyn JavaObject) -> Result<bool, MetadataError> {
        Ok(other.as_any().downcast_ref::<Self>() == Some(self))
    }
    fn java_hash_code(&self) -> Result<i32, MetadataError> {
        Ok(JavaString::java_hash_code(self))
    }
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        Ok(Some(self.clone()))
    }
}
