use crate::{DbError, MySqlRecoveryError};
use seata_at::LockKeyError;
#[cfg(feature = "experimental_changed_commit")]
use seata_protocol::{
    transaction::BranchReportRequest, transaction::BranchReportResponse, BranchStatus,
};
#[cfg(feature = "experimental_changed_commit")]
use seata_rm::ResourceClient;
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
    /// 实际镜像无法无损生成原锁键，或原键超出所选 TC 锁存储的约束。
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
        /// 原业务事务显式回滚结果；错误或期限表示回滚未确认，不允许发送失败报告。
        rollback: Result<(), Box<DbError>>,
        /// 真实错误作用域持有的单次失败报告门闩，取消等待后不能再次发送。
        report_guard: crate::MySqlFailureReportGuard,
    },
    /// 已登记的分支未能完成原物理事务内的 Undo 写入。
    #[error("MySQL preparation Undo failed: {source}")]
    Undo {
        /// 原真实回执，供原分支核对。
        receipt: Box<RegisteredBranch>,
        /// 脱敏原数据库／编码错误。
        source: DbError,
        /// 原业务事务显式回滚结果；错误或期限表示回滚未确认，不允许发送失败报告。
        rollback: Result<(), Box<DbError>>,
        /// 真实错误作用域持有的单次失败报告门闩，取消等待后不能再次发送。
        report_guard: crate::MySqlFailureReportGuard,
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
    /// 无参数；仅已登记后的准备失败且原连接显式回滚成功时返回 true。
    /// 期限取消与事务 Drop 不构成成功确认；调用方据此决定是否可报告 PhaseOne_Failed。
    pub fn local_rollback_confirmed(&self) -> bool {
        matches!(
            self,
            Self::Enrollment {
                rollback: Ok(()),
                ..
            } | Self::Undo {
                rollback: Ok(()),
                ..
            }
        )
    }

    /// 无参数；返回已尝试的原业务事务回滚错误；未尝试或成功时返回 None。
    pub fn rollback_error(&self) -> Option<&DbError> {
        match self {
            Self::Enrollment {
                rollback: Err(source),
                ..
            }
            | Self::Undo {
                rollback: Err(source),
                ..
            } => Some(source.as_ref()),
            _ => None,
        }
    }

    /// 无参数；返回此真实准备错误是否已进入一次失败报告调用，不证明 TC 已收到。
    pub fn failure_report_started(&self) -> bool {
        match self {
            Self::Enrollment { report_guard, .. } | Self::Undo { report_guard, .. } => {
                report_guard.started()
            }
            _ => false,
        }
    }

    #[cfg(feature = "experimental_changed_commit")]
    /// 参数为原 RM 客户端；只在真实回执及显式回滚成功后单次报告 PhaseOne_Failed。
    /// 返回原 TC 响应；明确拒绝与未知结果保留原 RmCallError。调用取消仍消耗此门闩。
    pub async fn report_failure_after_rollback(
        &self,
        client: &ResourceClient,
    ) -> Result<BranchReportResponse, crate::MySqlPrepareFailureReportError> {
        let (receipt, guard) = match self {
            Self::Enrollment {
                receipt,
                rollback: Ok(()),
                report_guard,
                ..
            }
            | Self::Undo {
                receipt,
                rollback: Ok(()),
                report_guard,
                ..
            } => (receipt, report_guard),
            _ => return Err(crate::MySqlPrepareFailureReportError::NotEligible),
        };
        let identity = receipt.identity();
        let request = BranchReportRequest {
            xid: Some(identity.xid().to_owned()),
            branch_id: identity.branch_id(),
            status: BranchStatus::PhaseOneFailed,
            resource_id: Some(identity.resource().resource_id().to_owned()),
            application_data: identity.application_data().map(str::to_owned),
            branch_type: identity.resource().branch_type(),
        };
        if !guard.try_start() {
            return Err(crate::MySqlPrepareFailureReportError::AlreadyAttempted);
        }
        client.branch_report(request).await.map_err(Into::into)
    }
}
