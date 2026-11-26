use crate::{DbError, MySqlRecoveryError};
use seata_at::LockKeyError;
use seata_rm::{RegisteredBranch, RmCallError};

/// 一阶段准备错误：保留真实已知回执，分开注册未知、控制登记失败和原 Undo 写入失败。
/// 对应 Java: org.apache.seata.rm.datasource.ConnectionProxy#processGlobalTransactionCommit 的错误路径；Rust 增加显式准备交接。
#[derive(Debug, thiserror::Error)]
pub enum MySqlPrepareError {
    /// 原作用域或输入校验失败；没有发送本次分支申请。
    #[error("local MySQL preparation failed: {source}")]
    Local {
        /// 脱敏本地错误。
        source: DbError,
    },
    /// 实际镜像无法无损生成原锁键。
    #[error("MySQL preparation lock key failed: {0}")]
    LockKey(#[from] LockKeyError),
    /// 正式单次注册的原始分类，不补造编号。
    #[error("MySQL preparation registration failed: {0}")]
    Registration(RmCallError),
    /// 已取得真实成功回执，但独立控制登记没有确认完成。
    #[error("MySQL preparation ownership failed: {source}")]
    Enrollment {
        /// 原真实回执；不授权重新注册或重放业务。
        receipt: Box<RegisteredBranch>,
        /// 控制登记的提交前／提交未知分类。
        source: MySqlRecoveryError,
    },
    /// 已登记的分支未能完成原物理事务内的 Undo 写入。
    #[error("MySQL preparation Undo failed: {source}")]
    Undo {
        /// 原真实回执，供原分支核对。
        receipt: Box<RegisteredBranch>,
        /// 脱敏原数据库／编码错误。
        source: DbError,
    },
    /// 入口绝对期限已到；原事务被消耗，独立控制登记可能已生效。
    #[error("MySQL preparation absolute deadline exceeded")]
    Deadline {
        /// 已取得时保留真实回执，否则不合成身份。
        receipt: Option<Box<RegisteredBranch>>,
        /// 注册等待已经开始，取消时不能证明原业务帧未发送。
        registration_may_have_effect: bool,
    },
}
impl MySqlPrepareError {
    /// 无参数；返回已经真实取得的注册回执；None 不证明 TC 没有登记。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        match self {
            Self::Enrollment { receipt, .. } | Self::Undo { receipt, .. } => Some(receipt),
            Self::Deadline { receipt, .. } => receipt.as_deref(),
            _ => None,
        }
    }
    /// 无参数；返回是否缺少注册权威结果；不表示本地业务发生 COMMIT，也不允许重发。
    pub fn is_registration_unknown(&self) -> bool {
        match self {
            Self::Registration(source) => source.is_unknown_outcome(),
            Self::Deadline {
                receipt: None,
                registration_may_have_effect: true,
            } => true,
            _ => false,
        }
    }
}
