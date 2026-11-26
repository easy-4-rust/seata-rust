use crate::DbError;
use seata_rm::RegisteredBranch;

/// 无变化一阶段提交错误；区分发送前不适用与 COMMIT 可能已生效的未知结果。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 的无分支本地提交边界。
#[derive(Debug, thiserror::Error)]
pub enum MySqlNoChangeCommitError {
    /// 已存在真实登记回执、Undo 或见证；未发送 COMMIT，原事务随作用域丢弃。
    #[error("no-change COMMIT is not eligible")]
    NotEligible {
        /// 已取得时保留原真实登记回执，调用方不得把它当作未登记。
        receipt: Option<Box<RegisteredBranch>>,
    },
    /// COMMIT 已进入驱动调用；结果未知，不能重放原业务 SQL。
    #[error("no-change COMMIT outcome unknown: {source}")]
    Unknown {
        /// 原驱动诊断，不包含业务 SQL 与凭据。
        #[source]
        source: DbError,
    },
}
impl MySqlNoChangeCommitError {
    /// 无参数；返回不适用分支的原登记回执；未知结果或无回执时为 None。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        match self {
            Self::NotEligible { receipt } => receipt.as_deref(),
            Self::Unknown { .. } => None,
        }
    }

    /// 无参数；返回 COMMIT 是否可能已经发送并生效。
    pub const fn is_unknown_outcome(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }
}
