use crate::{JavaString, JavaThrowable, MetadataError};
use std::{error::Error, fmt, sync::Arc};

/// SQL 解析错误，保留可空消息及原始原因身份；不把未知原因替换为空文本。
/// 对应 Java: org.apache.seata.sqlparser.SQLParsingException
#[derive(Debug, Clone)]
pub struct SQLParsingException {
    message: Option<JavaString>,
    cause: Option<Arc<dyn Error + Send + Sync>>,
}

impl SQLParsingException {
    /// 接收可空 message，返回没有原因的错误。对应 Java: SQLParsingException(String)
    pub fn new(message: Option<String>) -> Self {
        Self::with_cause(message, None)
    }

    /// 接收独立的可空 message 和 cause 并保留原身份，返回解析错误。
    /// 对应 Java: SQLParsingException(String, Throwable)
    pub fn with_cause(
        message: Option<String>,
        cause: Option<Arc<dyn Error + Send + Sync>>,
    ) -> Self {
        Self::with_java_message(message.map(Into::into), cause)
    }

    /// 接收可空 cause；其 Display 作为消息，空原因对应空消息，返回解析错误。
    /// 对应 Java: SQLParsingException(Throwable)；Java Throwable.toString 映射原因的 Display。
    pub fn from_cause(cause: Option<Arc<dyn Error + Send + Sync>>) -> Self {
        let message = cause.as_ref().map(ToString::to_string);
        Self::with_java_message(message.map(Into::into), cause)
    }

    /// 返回UTF-8便利消息视图；null或孤立代理码元返回None，无参数；无损读取用message_java。
    /// 对应 Java: Throwable#getMessage 的Rust便利视图，原始Java语义由message_java提供。
    pub fn message(&self) -> Option<&str> {
        self.message.as_ref().and_then(JavaString::as_str)
    }

    /// 返回原原因的共享引用，可用 Arc::ptr_eq 验证身份；无参数。对应 Java: Throwable#getCause
    pub fn cause(&self) -> Option<&Arc<dyn Error + Send + Sync>> {
        self.cause.as_ref()
    }

    /// 接收可空原始 message 和 cause，返回无损解析错误。对应 Java: SQLParsingException(String,Throwable)
    pub fn with_java_message(
        message: Option<JavaString>,
        cause: Option<Arc<dyn Error + Send + Sync>>,
    ) -> Self {
        Self { message, cause }
    }

    /// 用可空原因的无损 Java 文本构造错误，保留原因身份。对应 Java: SQLParsingException(Throwable)
    /// 若原因toString抛异常则返回同一原异常，不构造新的解析错误。
    pub fn from_java_cause<C: JavaThrowable>(cause: Option<Arc<C>>) -> Result<Self, MetadataError> {
        let message = cause
            .as_ref()
            .map(|value| value.java_to_string())
            .transpose().unwrap_or(None)
            .flatten();
        let cause = cause.map(|value| value as Arc<dyn Error + Send + Sync>);
        Ok(Self { message, cause })
    }

    /// 返回原始可空消息的借用，无参数且无损。对应 Java: Throwable#getMessage
    pub fn message_java(&self) -> Option<&JavaString> {
        self.message.as_ref()
    }

    /// 返回完整 Java toString 文本，保留孤立代理码元，无参数。对应 Java: Throwable#toString
    pub fn java_to_string(&self) -> JavaString {
        let mut units: Vec<u16> = "org.apache.seata.sqlparser.SQLParsingException"
            .encode_utf16()
            .collect();
        if let Some(message) = &self.message {
            units.extend(": ".encode_utf16());
            units.extend(message.as_utf16());
        }
        JavaString::from_utf16(units)
    }
}

impl fmt::Display for SQLParsingException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("org.apache.seata.sqlparser.SQLParsingException")?;
        if let Some(message) = &self.message {
            write!(f, ": {message}")?;
        }
        Ok(())
    }
}

impl Error for SQLParsingException {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}
