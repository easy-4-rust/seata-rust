use crate::JavaString;
use std::error::Error;

/// Rust 扩展：为原因对象提供无损 Java toString 文本，原错误身份仍由 Arc 保留。
/// 对应 Java: java.lang.Throwable#toString；接口允许子类返回 null，不冒充完整 Throwable 移植。
pub trait JavaThrowable: Error + Send + Sync + 'static {
    /// 返回原因的原始 Java 文本或 null；无参数，禁止先经有损 UTF-8 转换。
    fn java_to_string(&self) -> Option<JavaString>;
}
