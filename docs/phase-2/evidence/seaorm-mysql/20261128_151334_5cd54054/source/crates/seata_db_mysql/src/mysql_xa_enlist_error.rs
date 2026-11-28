use crate::{DbError, MySqlXaError};
use seata_rm::{RegisteredBranch, RmCallError};
/// XA 登记/受管理作用域失败保留原成功回执，不从未知申请合成身份。
/// 对应 Java: ConnectionProxyXA 的登记及 Prepare 异常边界；Rust 明确区分已知原身份。
#[derive(Debug, thiserror::Error)]
pub enum MySqlXaEnlistError {
    /// 原申请尚未登记，配置/输入不满足当前受管理模式。
    #[error("XA enrollment rejected before registration")]
    Local {
        /// 固定本地原因。
        #[source]
        source: DbError,
    },
    /// 正式 RM 单次登记失败；沿用原请求可能已生效语义，不重发或伪造编号。
    #[error(transparent)]
    Registration(#[from] RmCallError),
    /// 原登记已确认，但数据库控制结果不能确认或不允许继续。
    #[error("XA registered control operation failed")]
    Control {
        /// 原成功登记回执。
        receipt: Box<RegisteredBranch>,
        /// 脱敏控制失败原因。
        #[source]
        source: DbError,
    },
    /// 原登记已确认，原生 START/DML/Prepare 未确认或被拒绝，不自动重新启动分支。
    #[error("XA registered native operation failed")]
    Native {
        /// 原成功登记回执。
        receipt: Box<RegisteredBranch>,
        /// 原生发送/未知边界。
        #[source]
        source: MySqlXaError,
    },
    /// 绝对期限已到；有原回执时保留，尚无回执时也不否定登记可能已生效。
    #[error("XA enrollment operation deadline exceeded")]
    Deadline {
        /// 已确认的原登记回执。
        receipt: Option<Box<RegisteredBranch>>,
        /// 单次登记是否可能已经发送。
        registration_may_have_effect: bool,
    },
}
impl MySqlXaEnlistError {
    /// 无参数；返回已经真实取得的原回执；None 不代表注册确定未生效。
    pub fn registered_branch(&self) -> Option<&RegisteredBranch> {
        match self {
            Self::Control { receipt, .. } | Self::Native { receipt, .. } => Some(receipt),
            Self::Deadline { receipt, .. } => receipt.as_deref(),
            Self::Local { .. } | Self::Registration(_) => None,
        }
    }
}
