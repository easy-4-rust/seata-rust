use crate::{JavaString, JavaThrowable, MetadataError};
use std::{
    collections::HashSet,
    error::Error,
    fmt,
    sync::{Arc, Mutex, PoisonError},
};

static SUPPRESSION_GRAPH: Mutex<()> = Mutex::new(());

/// Rust扩展：动态Java对象方法抛出的异常，类名、原消息及Arc身份独立保留。
/// 对应 Java: Object/Throwable回调抛异常的边界；不计入31个源对象。
pub struct JavaObjectError {
    class_name: String,
    superclasses: Vec<String>,
    message: Option<JavaString>,
    cause: Option<Arc<dyn Error + Send + Sync>>,
    suppressed: Mutex<Vec<Arc<JavaObjectError>>>,
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
            suppressed: Mutex::new(Vec::new()),
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

    /// 返回当前 suppressed 异常的有序快照，保留各异常的共享身份。
    /// 对应 Java: Throwable#getSuppressed。
    pub fn suppressed(&self) -> Vec<Arc<Self>> {
        self.suppressed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 添加清理失败 suppressed。附加自身返回带原因的异常，Rust 拒绝会形成强引用环的异常图。
    /// 对应 Java: Throwable#addSuppressed。
    pub fn add_suppressed(self: &Arc<Self>, suppressed: Arc<Self>) -> Result<(), MetadataError> {
        if Arc::ptr_eq(self, &suppressed) {
            return Err(MetadataError::ObjectException(Arc::new(
                Self::with_cause(
                    "java.lang.IllegalArgumentException",
                    Some("Self-suppression not permitted".into()),
                    Some(self.clone()),
                )
                .with_superclasses([
                    "java.lang.RuntimeException",
                    "java.lang.Exception",
                    "java.lang.Throwable",
                ]),
            )));
        }
        // 串行检查与插入，防止两个线程互相附加时同时越过检测。
        let _graph = SUPPRESSION_GRAPH
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if reaches(&suppressed, self) {
            return Err(MetadataError::InvalidCompatibilityData(
                "cyclic Java throwable suppression",
            ));
        }
        self.suppressed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(suppressed);
        Ok(())
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

fn reaches(from: &Arc<JavaObjectError>, target: &JavaObjectError) -> bool {
    let mut visited = HashSet::new();
    let mut pending: Vec<Arc<dyn Error + Send + Sync>> = vec![from.clone()];
    while let Some(reference) = pending.pop() {
        if let Some(metadata) = reference.downcast_ref::<MetadataError>() {
            let mut errors = vec![metadata];
            while let Some(error) = errors.pop() {
                match error {
                    MetadataError::ObjectException(error) => pending.push(error.clone()),
                    MetadataError::CleanupFailed { primary, cleanup } => {
                        errors.extend([primary.as_ref(), cleanup.as_ref()]);
                    }
                    _ => {}
                }
            }
            continue;
        }
        let Some(error) = reference.downcast_ref::<JavaObjectError>() else {
            continue;
        };
        if std::ptr::eq(error, target) {
            return true;
        }
        if visited.insert(std::ptr::from_ref(error)) {
            pending.extend(error.cause.iter().cloned());
            pending.extend(
                error
                    .suppressed()
                    .into_iter()
                    .map(|suppressed| suppressed as Arc<dyn Error + Send + Sync>),
            );
        }
    }
    false
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
