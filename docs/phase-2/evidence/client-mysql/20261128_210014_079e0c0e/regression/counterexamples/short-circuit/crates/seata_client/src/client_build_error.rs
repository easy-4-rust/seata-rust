use seata_rm::RmError;
use seata_tm::TmError;
use std::fmt;

/// 入口配置、RM 启动或 TM 启动及后续 RM 清理错误；诊断默认脱敏。
/// 对应 Java: GlobalTransactionScanner 初始化异常；Rust 生命周期扩展。
pub struct ClientBuildError {
    pub(crate) configuration: Option<&'static str>,
    pub(crate) tm: Option<Box<TmError>>,
    pub(crate) rm: Option<RmError>,
}
impl ClientBuildError {
    pub(crate) fn invalid(reason: &'static str) -> Self {
        Self {
            configuration: Some(reason),
            tm: None,
            rm: None,
        }
    }
    /// 无参数；返回入口校验的静态原因，未失败于入口校验时返回 None。
    pub fn configuration_error(&self) -> Option<&'static str> {
        self.configuration
    }
    /// 无参数；返回原 TM 启动错误的借用，供受控诊断。
    pub fn tm_error(&self) -> Option<&TmError> {
        self.tm.as_deref()
    }
    /// 无参数；返回原 RM 启动或清理错误的借用，供受控诊断。
    pub fn rm_error(&self) -> Option<&RmError> {
        self.rm.as_ref()
    }
}
impl fmt::Display for ClientBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if let Some(reason) = self.configuration {
            reason
        } else if self.tm.is_some() && self.rm.is_some() {
            "TM startup and RM cleanup failed"
        } else if self.tm.is_some() {
            "TM startup failed"
        } else {
            "RM startup failed"
        })
    }
}
impl fmt::Debug for ClientBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for ClientBuildError {}
