use seata_rm::RmError;
use seata_tm::TmError;
use std::fmt;

/// 两个角色关闭后的独立错误；其中一方失败不阻止另一方清理。
/// 对应 Java: GlobalTransactionScanner 销毁职责；Rust 显式关闭扩展。
pub struct ClientShutdownError {
    pub(crate) tm: Option<TmError>,
    pub(crate) rm: Option<RmError>,
}
impl ClientShutdownError {
    /// 无参数；返回 TM 原关闭错误的借用，未失败则返回 None。
    pub fn tm_error(&self) -> Option<&TmError> {
        self.tm.as_ref()
    }
    /// 无参数；返回 RM 原关闭错误的借用，未失败则返回 None。
    pub fn rm_error(&self) -> Option<&RmError> {
        self.rm.as_ref()
    }
}
impl fmt::Display for ClientShutdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match (self.tm.is_some(), self.rm.is_some()) {
            (true, true) => "TM and RM shutdown failed",
            (true, false) => "TM shutdown failed",
            (false, true) => "RM shutdown failed",
            (false, false) => "client shutdown failed",
        })
    }
}
impl fmt::Debug for ClientShutdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for ClientShutdownError {}
