/// 精确物理恢复表的能力版本；旧格式继续只接受已知注册回执。
/// 对应 Java: 无直接对象；WP05 Rust 独立恢复表的显式扩展迁移标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoverySchema {
    Legacy,
    CallbackOrigin,
    CommitDecision,
    FirstPhaseWitness,
    UndoChecksum,
}
impl RecoverySchema {
    /// 返回是否包含一阶段见证字段。
    pub(crate) const fn has_witness(self) -> bool {
        matches!(self, Self::FirstPhaseWitness | Self::UndoChecksum)
    }

    /// 无参数；返回恢复表是否含 003 二阶段授权字段。
    pub(crate) const fn has_commit_decision(self) -> bool {
        matches!(
            self,
            Self::CommitDecision | Self::FirstPhaseWitness | Self::UndoChecksum
        )
    }
}
