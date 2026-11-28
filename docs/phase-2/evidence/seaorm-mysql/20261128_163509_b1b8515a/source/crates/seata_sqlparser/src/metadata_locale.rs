use crate::{JavaString, JdkCasing, MetadataError};
use std::sync::{Arc, RwLock};

/// Rust扩展：由宿主显式注入的元数据语言上下文，Clone共享后续语言变更。
/// 对应 Java: Locale.getDefault的语言输入边界；尚不宣称系统默认Locale启动发现已经迁移。
#[derive(Clone, Debug)]
pub struct MetadataLocale {
    language: Arc<RwLock<String>>,
}

impl MetadataLocale {
    /// 用language规范化语言值建立共享上下文并返回；不读取或更改宿主系统Locale。
    /// 对应 Java: 已初始化的Locale#getLanguage输入；Rust显式依赖注入扩展。
    pub fn new(language: &str) -> Self {
        Self {
            language: Arc::new(RwLock::new(language.to_ascii_lowercase())),
        }
    }
    /// 返回当前规范化语言，无参数；共享锁中毒明确返回错误。
    /// 对应 Java: Locale#getLanguage
    pub fn language(&self) -> Result<String, MetadataError> {
        Ok(self
            .language
            .read()
            .map_err(|_| MetadataError::LockPoisoned("MetadataLocale"))?
            .clone())
    }
    /// 用language替换该共享上下文并返回原值；不宣称修改整个进程的Java Locale全局状态。
    /// 对应 Java: Locale#setDefault的显式上下文变更边界，宿主默认值集成仍待完成。
    pub fn set_language(&self, language: &str) -> Result<String, MetadataError> {
        let mut current = self
            .language
            .write()
            .map_err(|_| MetadataError::LockPoisoned("MetadataLocale"))?;
        Ok(std::mem::replace(
            &mut *current,
            language.to_ascii_lowercase(),
        ))
    }
    /// 按调用时的共享语言将value转小写并返回原始UTF16；数据或锁错误返回失败。
    /// 对应 Java: String#toLowerCase()调用时读取Locale.getDefault。
    pub fn lower(&self, value: &JavaString) -> Result<JavaString, MetadataError> {
        JdkCasing::lower_case(value, &self.language()?)
    }
    /// 按调用时共享语言将value转大写并返回原始UTF16；保留展开和条件映射。
    /// 对应 Java: String#toUpperCase()调用时读取Locale.getDefault。
    pub fn upper(&self, value: &JavaString) -> Result<JavaString, MetadataError> {
        JdkCasing::upper_case(value, &self.language()?)
    }
}
