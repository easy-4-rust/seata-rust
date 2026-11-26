use crate::JavaString;
use std::{any::Any, fmt::Debug};

/// Rust 扩展：元数据 Object 字段的动态语义桥，允许调用方适配任意真实对象类型。
/// 对应 Java: java.lang.Object 的 equals/hashCode/toString；不冒充完整 Object 移植。
pub trait JavaObject: Debug + Send + Sync + 'static {
    /// 返回动态类型借用，无参数；用于保留 Integer、Long 等不同运行类型。
    fn as_any(&self) -> &dyn Any;
    /// 比较指定 other 的 Java 对象语义；不把不同数值类型自动视为相等。
    fn java_equals(&self, other: &dyn JavaObject) -> bool;
    /// 返回原对象的32位 Java hashCode；无参数。
    fn java_hash_code(&self) -> i32;
    /// 返回可空原 Java toString 文本；无参数，保留孤立代理码元。
    fn java_to_string(&self) -> Option<JavaString>;
}

macro_rules! integral {
    ($type:ty, $hash:expr) => {
        impl JavaObject for $type {
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn java_equals(&self, other: &dyn JavaObject) -> bool {
                other.as_any().downcast_ref::<Self>() == Some(self)
            }
            fn java_hash_code(&self) -> i32 {
                ($hash)(*self)
            }
            fn java_to_string(&self) -> Option<JavaString> {
                Some(self.to_string().into())
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
    fn java_equals(&self, other: &dyn JavaObject) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }
    fn java_hash_code(&self) -> i32 {
        JavaString::java_hash_code(self)
    }
    fn java_to_string(&self) -> Option<JavaString> {
        Some(self.clone())
    }
}
