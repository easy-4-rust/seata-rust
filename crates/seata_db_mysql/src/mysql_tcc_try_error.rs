use crate::DbError;
use seata_rm::{RegisteredBranch, RmCallError};
/// TCC Try 登记与本地事务失败；已确认原回执永不丢失或重造。
/// 对应 Java: TccActionInterceptorHandler 的登记/prepareFence 异常边界；Rust 原回执扩展。
#[derive(Debug, thiserror::Error)]
pub enum MySqlTccTryError {
    /// 原登记前的输入或配置拒绝。
    #[error("TCC Try rejected before registration")]
    Local {
        /// 脱敏本地原因。
        #[source]
        source: DbError,
    },
    /// 正式单次登记失败，保留请求可能生效语义。
    #[error(transparent)]
    Registration(#[from] RmCallError),
    /// 本次 Try 没有发送 COMMIT，事务被拒绝或关闭；不否定其他尝试已有的 Fence，也不授权重放。
    #[error("TCC registered Try refused before commit")]
    Refused {
        /// 原 TC 成功登记回执。
        receipt: Box<RegisteredBranch>,
        /// 脱敏本地原因。
        #[source]
        source: DbError,
    },
    /// 本次 Try 的 COMMIT 可能已发送，但响应未确认；只能保留原身份核对 Fence。
    #[error("TCC Try commit outcome unknown")]
    UnknownCommit {
        /// 原 TC 成功登记回执。
        receipt: Box<RegisteredBranch>,
        /// 脱敏本地原因。
        #[source]
        source: DbError,
    },
    /// 绝对期限，未知登记不能合成分支或否定原请求。
    #[error("TCC Try operation deadline exceeded")]
    Deadline {
        /// 已真实取得的原回执。
        receipt: Option<Box<RegisteredBranch>>,
        /// 单次登记是否可能已发送。
        registration_may_have_effect: bool,
        /// 本次 Try 的 COMMIT 是否可能已经发送；false 不否定其他尝试或全局结果。
        try_commit_may_have_effect: bool,
    },
}
impl MySqlTccTryError {
    /// 无参数；返回真实原登记借用，None 不表示登记必定未生效。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        match self {
            Self::Refused { receipt, .. } | Self::UnknownCommit { receipt, .. } => Some(receipt),
            Self::Deadline { receipt, .. } => receipt.as_deref(),
            _ => None,
        }
    }
    /// 参数为原成功回执、脱敏原因和本次实际提交发送边界；返回明确拒绝或提交未知，不丢失原身份。
    pub(crate) fn transaction(
        receipt: RegisteredBranch,
        source: DbError,
        commit_started: bool,
    ) -> Self {
        if commit_started {
            Self::UnknownCommit {
                receipt: Box::new(receipt),
                source,
            }
        } else {
            Self::Refused {
                receipt: Box::new(receipt),
                source,
            }
        }
    }
}
