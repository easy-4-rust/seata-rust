use crate::MySqlPhaseOneOutcome;
/// 一阶段编排的固定阶段，嵌套阶段分别计量，不能相加当作业务效果次数。
/// 对应 Java: 无直接对象；WP07 Rust 一阶段编排调用观测扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum MySqlPhaseOneOperation {
    /// 实际登记与 Undo 准备阶段。
    Prepare,
    /// 原事务提交前检查与暂存见证。
    CheckCommit,
    /// 无变化本地 COMMIT 入口。
    CommitNoChange,
    /// 实验有变化 COMMIT 及独立见证确认入口。
    CommitChanged,
    /// 实验本地提交及单次成功报告组合入口。
    CommitAndReport,
}
impl MySqlPhaseOneOperation {
    /// 固定全部阶段；实验阶段为零不表示相应能力已经启用。
    pub const ALL: [Self; 5] = [
        Self::Prepare,
        Self::CheckCommit,
        Self::CommitNoChange,
        Self::CommitChanged,
        Self::CommitAndReport,
    ];
    /// 无参数；返回固定阶段标签，不含业务身份。
    pub const fn label(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::CheckCommit => "check_commit",
            Self::CommitNoChange => "commit_no_change",
            Self::CommitChanged => "commit_changed",
            Self::CommitAndReport => "commit_and_report",
        }
    }
    /// 无参数；返回本阶段适用的固定结果，用于有界导出。
    pub fn outcomes(self) -> &'static [MySqlPhaseOneOutcome] {
        use MySqlPhaseOneOutcome as O;
        match self {
            Self::Prepare => &[
                O::PreparedUnregistered,
                O::PreparedRegistered,
                O::RegistrationUnknown,
                O::PrepareFailed,
                O::Cancelled,
            ],
            Self::CheckCommit => &[
                O::CheckedWithoutWitness,
                O::CheckedWithWitness,
                O::CheckFailed,
                O::Cancelled,
            ],
            Self::CommitNoChange => &[
                O::NoChangeCommitted,
                O::NoChangeCommitUnknown,
                O::NoChangeNotEligible,
                O::Cancelled,
            ],
            Self::CommitChanged => &[
                O::ChangedCommitted,
                O::ChangedCommitUnknown,
                O::ChangedNotEligible,
                O::ArmingFailed,
                O::ArmingUncertain,
                O::Cancelled,
            ],
            Self::CommitAndReport => &[
                O::ReportObserved,
                O::ReportLocalFailed,
                O::ReportLocalUncertain,
                O::ReportClaimFailed,
                O::ReportUnknown,
                O::ReportFailed,
                O::Cancelled,
            ],
        }
    }
}
