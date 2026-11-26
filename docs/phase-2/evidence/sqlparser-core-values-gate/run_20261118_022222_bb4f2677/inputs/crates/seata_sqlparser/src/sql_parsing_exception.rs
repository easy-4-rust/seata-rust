use std::{error::Error, fmt, sync::Arc};

/// SQL 解析错误，保留可空消息及原始原因身份；不把未知原因替换为空文本。
/// 对应 Java: org.apache.seata.sqlparser.SQLParsingException
#[derive(Debug, Clone)]
pub struct SQLParsingException {
    message: Option<String>,
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
        Self { message, cause }
    }

    /// 接收可空 cause；其 Display 作为消息，空原因对应空消息，返回解析错误。
    /// 对应 Java: SQLParsingException(Throwable)；Java Throwable.toString 映射原因的 Display。
    pub fn from_cause(cause: Option<Arc<dyn Error + Send + Sync>>) -> Self {
        let message = cause.as_ref().map(ToString::to_string);
        Self { message, cause }
    }

    /// 返回原消息的借用，区分 None 与空字符串；无参数。对应 Java: Throwable#getMessage
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// 返回原原因的共享引用，可用 Arc::ptr_eq 验证身份；无参数。对应 Java: Throwable#getCause
    pub fn cause(&self) -> Option<&Arc<dyn Error + Send + Sync>> {
        self.cause.as_ref()
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
