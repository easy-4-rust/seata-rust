/// 一阶段编排 API 的固定结果，保留业务提交、控制登记及报告的不确定性边界。
/// 对应 Java: 无直接对象；WP07 Rust 一阶段编排调用观测扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum MySqlPhaseOneOutcome {
    /// 准备返回但无登记回执，不代表业务已提交。
    PreparedUnregistered,
    /// 准备返回真实登记回执，不代表本地已提交。
    PreparedRegistered,
    /// 准备阶段缺少注册权威结果，不允许重发。
    RegistrationUnknown,
    /// 准备阶段返回其他错误，不推断旧事务效果。
    PrepareFailed,
    /// 提交前检查返回，未暂存提交见证。
    CheckedWithoutWitness,
    /// 提交前检查返回且暂存见证，尚未提交。
    CheckedWithWitness,
    /// 提交前检查失败，不声称已发送业务 COMMIT。
    CheckFailed,
    /// 无变化作用域原 COMMIT 返回成功。
    NoChangeCommitted,
    /// 无变化原 COMMIT 确认未知。
    NoChangeCommitUnknown,
    /// 无变化提交不适用，本次未发送 COMMIT。
    NoChangeNotEligible,
    /// 有变化 COMMIT 及独立见证均已确认。
    ChangedCommitted,
    /// 有变化 COMMIT 或独立见证未获整体确认。
    ChangedCommitUnknown,
    /// 有变化提交不适用，本次未发送 COMMIT。
    ChangedNotEligible,
    /// 尝试记录失败且回滚已确认，本次业务 COMMIT 未发送。
    ArmingFailed,
    /// 尝试记录或回滚不确定，本次业务 COMMIT 未发送。
    ArmingUncertain,
    /// 本地提交已确认且成功报告 API 返回，非全局提交证明。
    ReportObserved,
    /// 组合入口的本地阶段失败，未进入成功报告。
    ReportLocalFailed,
    /// 组合入口的本地阶段不确定，未进入成功报告。
    ReportLocalUncertain,
    /// 本地提交已确认，但未获新的报告发送权，不能重报。
    ReportClaimFailed,
    /// 本地提交已确认，单次报告结果未知。
    ReportUnknown,
    /// 本地提交已确认，单次报告返回其他错误。
    ReportFailed,
    /// 已轮询阶段取消或 panic 解栈，不推断提交边界。
    Cancelled,
}
pub(crate) const OUTCOME_COUNT: usize = MySqlPhaseOneOutcome::Cancelled as usize + 1;
impl MySqlPhaseOneOutcome {
    /// 无参数；返回固定指标标签，不含用户输入。
    pub const fn label(self) -> &'static str {
        match self {
            Self::PreparedUnregistered => "prepared_unregistered",
            Self::PreparedRegistered => "prepared_registered",
            Self::RegistrationUnknown => "registration_unknown",
            Self::PrepareFailed => "prepare_failed",
            Self::CheckedWithoutWitness => "checked_without_witness",
            Self::CheckedWithWitness => "checked_with_witness",
            Self::CheckFailed => "check_failed",
            Self::NoChangeCommitted => "no_change_committed",
            Self::NoChangeCommitUnknown => "no_change_commit_unknown",
            Self::NoChangeNotEligible => "no_change_not_eligible",
            Self::ChangedCommitted => "changed_committed",
            Self::ChangedCommitUnknown => "changed_commit_unknown",
            Self::ChangedNotEligible => "changed_not_eligible",
            Self::ArmingFailed => "arming_failed",
            Self::ArmingUncertain => "arming_uncertain",
            Self::ReportObserved => "report_observed",
            Self::ReportLocalFailed => "report_local_failed",
            Self::ReportLocalUncertain => "report_local_uncertain",
            Self::ReportClaimFailed => "report_claim_failed",
            Self::ReportUnknown => "report_unknown",
            Self::ReportFailed => "report_failed",
            Self::Cancelled => "cancelled",
        }
    }
}
