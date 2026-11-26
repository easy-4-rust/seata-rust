/// 精确物理恢复表的能力版本；旧格式继续只接受已知注册回执。
/// 对应 Java: 无直接对象；WP05 Rust 独立恢复表的显式扩展迁移标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoverySchema {
    Legacy,
    CallbackOrigin,
    CommitDecision,
}
impl RecoverySchema {
    /// 无参数；返回新插入行的逻辑版本，不改写既有行的版本。
    pub(crate) const fn new_record_version(self) -> u16 {
        match self {
            Self::Legacy => 1,
            Self::CallbackOrigin => 2,
            Self::CommitDecision => 3,
        }
    }
}
