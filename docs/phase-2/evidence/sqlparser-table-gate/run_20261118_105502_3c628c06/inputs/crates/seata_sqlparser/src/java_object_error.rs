use crate::{JavaString, JavaThrowable, MetadataError};
use std::{error::Error, fmt, sync::Arc};

/// Rust扩展：动态Java对象方法抛出的异常，类名、原消息及Arc身份独立保留。
/// 对应 Java: Object/Throwable回调抛异常的边界；不计入31个源对象。
pub struct JavaObjectError {
    class_name: String,
    superclasses: Vec<String>,
    message: Option<JavaString>,
    cause: Option<Arc<dyn Error + Send + Sync>>,
}

impl JavaObjectError {
    /// 用class_name和nullable message建立异常；无原原因，返回可由Arc共享的值。
    /// 自定义异常参与继承catch时，适配器必须另外提供with_superclasses父类链。
    /// 对应 Java: 异常类身份与Throwable#getMessage
    pub fn new(class_name: impl Into<String>, message: Option<JavaString>) -> Self {
        Self::with_cause(class_name, message, None)
    }

    /// 保存class_name、原始message及nullable cause，返回异常；不调用用户格式化。
    /// 对应 Java: Throwable(String,Throwable)的异常边界数据
    pub fn with_cause(
        class_name: impl Into<String>,
        message: Option<JavaString>,
        cause: Option<Arc<dyn Error + Send + Sync>>,
    ) -> Self {
        Self {
            class_name: class_name.into(),
            superclasses: Vec::new(),
            message,
            cause,
        }
    }

    /// 返回原Java异常类名，无参数。
    /// 对应 Java: Class#getName
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// 返回原nullable UTF16消息，无参数，不以诊断文本替代。
    /// 对应 Java: Throwable#getMessage
    pub fn message_java(&self) -> Option<&JavaString> {
        self.message.as_ref()
    }

    /// 接收适配器从实际异常类型获得的superclasses父类链并返回本异常；保留类名、消息、原因。
    /// 对应 Java: Class#getSuperclass链；自定义异常不能只凭名字推断继承关系。
    pub fn with_superclasses(
        mut self,
        superclasses: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.superclasses = superclasses.into_iter().map(Into::into).collect();
        self
    }

    /// 判断class_name是否为本类或显式提供的父类，无参数外副作用；未知父类不凭类名猜测。
    /// 对应 Java: catch类型可赋值检查的数据边界；适配自定义异常须提供完整相关父类链。
    pub fn is_instance_of(&self, class_name: &str) -> bool {
        self.class_name == class_name || self.superclasses.iter().any(|parent| parent == class_name)
    }
}

impl PartialEq for JavaObjectError {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl Eq for JavaObjectError {}

impl fmt::Debug for JavaObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JavaObjectError")
            .field("class_name", &self.class_name)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for JavaObjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.class_name)?;
        if let Some(message) = &self.message {
            write!(f, ": {message}")?;
        }
        Ok(())
    }
}

impl Error for JavaObjectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}

impl JavaThrowable for JavaObjectError {
    fn java_to_string(&self) -> Result<Option<JavaString>, MetadataError> {
        let mut units: Vec<u16> = self.class_name.encode_utf16().collect();
        if let Some(message) = &self.message {
            units.extend(": ".encode_utf16());
            units.extend(message.as_utf16());
        }
        Ok(Some(JavaString::from_utf16(units)))
    }
}
