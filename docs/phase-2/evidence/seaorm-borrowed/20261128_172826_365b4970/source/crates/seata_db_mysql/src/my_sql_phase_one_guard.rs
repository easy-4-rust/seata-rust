#[cfg(feature = "experimental_changed_commit")]
use crate::{MySqlChangedCommitError, MySqlFirstPhaseReportError};
use crate::{
    MySqlCommitCheckError, MySqlCommitCheckedTransaction, MySqlNoChangeCommitError,
    MySqlPhaseOneMetricsSampler, MySqlPhaseOneOperation, MySqlPhaseOneOutcome as Outcome,
    MySqlPrepareError, MySqlPreparedTransaction,
};
#[cfg(feature = "experimental_changed_commit")]
use seata_rm::RegisteredBranch;

/// 一阶段作用域结算守卫，只读取原结果，不改变原错误、回执、事务或报告门闩。
/// 对应 Java: 无直接对象；WP07 Rust 一阶段编排调用观测扩展。
pub(crate) struct MySqlPhaseOneGuard {
    sampler: MySqlPhaseOneMetricsSampler,
    operation: MySqlPhaseOneOperation,
    finished: bool,
}
impl MySqlPhaseOneGuard {
    /// 参数为内部采样能力及实际阶段；返回独占结算守卫。
    pub(crate) fn new(
        sampler: MySqlPhaseOneMetricsSampler,
        operation: MySqlPhaseOneOperation,
    ) -> Self {
        Self {
            sampler,
            operation,
            finished: false,
        }
    }
    /// 参数为原准备结果引用；无返回值，不把已准备当作已提交。
    pub(crate) fn finish_prepare(
        self,
        result: &Result<MySqlPreparedTransaction, MySqlPrepareError>,
    ) {
        self.finish(match result {
            Ok(prepared) if prepared.registered_branch().is_some() => Outcome::PreparedRegistered,
            Ok(_) => Outcome::PreparedUnregistered,
            Err(error) if error.is_registration_unknown() => Outcome::RegistrationUnknown,
            Err(_) => Outcome::PrepareFailed,
        });
    }
    /// 参数为原提交前检查结果引用；无返回值，暂存见证不表示已提交。
    pub(crate) fn finish_check(
        self,
        result: &Result<MySqlCommitCheckedTransaction, MySqlCommitCheckError>,
    ) {
        self.finish(match result {
            Ok(checked) if checked.require_first_phase_commit_witness().is_ok() => {
                Outcome::CheckedWithWitness
            }
            Ok(_) => Outcome::CheckedWithoutWitness,
            Err(_) => Outcome::CheckFailed,
        });
    }
    /// 参数为原无变化提交结果引用；无返回值，区分拒绝与 COMMIT 未知。
    pub(crate) fn finish_no_change(self, result: &Result<(), MySqlNoChangeCommitError>) {
        self.finish(match result {
            Ok(()) => Outcome::NoChangeCommitted,
            Err(MySqlNoChangeCommitError::Unknown { .. }) => Outcome::NoChangeCommitUnknown,
            Err(MySqlNoChangeCommitError::NotEligible { .. }) => Outcome::NoChangeNotEligible,
        });
    }
    /// 参数为原有变化提交结果引用；无返回值，不把控制登记不确定当作业务 COMMIT 已发送。
    #[cfg(feature = "experimental_changed_commit")]
    pub(crate) fn finish_changed(self, result: &Result<RegisteredBranch, MySqlChangedCommitError>) {
        self.finish(match result {
            Ok(_) => Outcome::ChangedCommitted,
            Err(MySqlChangedCommitError::NotEligible { .. }) => Outcome::ChangedNotEligible,
            Err(MySqlChangedCommitError::Unknown { .. }) => Outcome::ChangedCommitUnknown,
            Err(error @ MySqlChangedCommitError::Arming { .. }) if error.is_unknown_outcome() => {
                Outcome::ArmingUncertain
            }
            Err(MySqlChangedCommitError::Arming { .. }) => Outcome::ArmingFailed,
        });
    }
    /// 参数为原组合提交/成功报告结果引用；无返回值，报告失败仍保留本地已确认事实。
    #[cfg(feature = "experimental_changed_commit")]
    pub(crate) fn finish_report(
        self,
        result: &Result<RegisteredBranch, MySqlFirstPhaseReportError>,
    ) {
        self.finish(match result {
            Ok(_) => Outcome::ReportObserved,
            Err(MySqlFirstPhaseReportError::Local(error)) if error.is_unknown_outcome() => {
                Outcome::ReportLocalUncertain
            }
            Err(MySqlFirstPhaseReportError::Local(_)) => Outcome::ReportLocalFailed,
            Err(MySqlFirstPhaseReportError::Claim { .. }) => Outcome::ReportClaimFailed,
            Err(MySqlFirstPhaseReportError::Report { source, .. })
                if source.is_unknown_outcome() =>
            {
                Outcome::ReportUnknown
            }
            Err(MySqlFirstPhaseReportError::Report { .. }) => Outcome::ReportFailed,
        });
    }
    /// 参数为原 DML 结果；无返回值，成功仅表示未提交作用域内的语句与镜像准备。
    pub(crate) fn finish_execute<T>(self, result: &Result<T, crate::DbError>) {
        self.finish(if result.is_ok() {
            Outcome::ExecutionCompleted
        } else {
            Outcome::ExecutionFailed
        });
    }
    /// 参数为原显式回滚结果；无返回值，失败不能确认回滚或全局结果。
    pub(crate) fn finish_rollback(self, result: &Result<(), crate::DbError>) {
        self.finish(if result.is_ok() {
            Outcome::RollbackConfirmed
        } else {
            Outcome::RollbackUnconfirmed
        });
    }
    /// 参数为原回滚/失败报告结果；保留本地回滚与报告的不同事实。
    #[cfg(feature = "experimental_changed_commit")]
    pub(crate) fn finish_rollback_report(
        self,
        result: &Result<Option<RegisteredBranch>, crate::MySqlPhaseOneFailureReportError>,
    ) {
        self.finish(match result {
            Ok(Some(_)) => Outcome::FailureReportObserved,
            Ok(None) => Outcome::FailureReportSkipped,
            Err(crate::MySqlPhaseOneFailureReportError::Local { .. }) => {
                Outcome::FailureRollbackUnconfirmed
            }
            Err(crate::MySqlPhaseOneFailureReportError::Report { source, .. })
                if source.is_unknown_outcome() =>
            {
                Outcome::FailureReportUnknown
            }
            Err(crate::MySqlPhaseOneFailureReportError::Report { .. }) => {
                Outcome::FailureReportFailed
            }
        });
    }
    /// 参数为原显式准备失败报告结果；重复门闩拒绝不算本次网络未知尝试。
    #[cfg(feature = "experimental_changed_commit")]
    pub(crate) fn finish_prepare_report(
        self,
        result: &Result<
            seata_protocol::transaction::BranchReportResponse,
            crate::MySqlPrepareFailureReportError,
        >,
    ) {
        self.finish(Self::prepare_report_outcome(result));
    }
    #[cfg(feature = "experimental_changed_commit")]
    fn prepare_report_outcome(
        result: &Result<
            seata_protocol::transaction::BranchReportResponse,
            crate::MySqlPrepareFailureReportError,
        >,
    ) -> Outcome {
        match result {
            Ok(_) => Outcome::FailureReportObserved,
            Err(crate::MySqlPrepareFailureReportError::NotEligible) => {
                Outcome::FailureReportNotEligible
            }
            Err(crate::MySqlPrepareFailureReportError::AlreadyAttempted) => {
                Outcome::FailureReportAlreadyAttempted
            }
            Err(crate::MySqlPrepareFailureReportError::Call(source))
                if source.is_unknown_outcome() =>
            {
                Outcome::FailureReportUnknown
            }
            Err(crate::MySqlPrepareFailureReportError::Call(_)) => Outcome::FailureReportFailed,
        }
    }
    /// 参数为组合准备/失败报告的原结果；未报告不表示本地业务已安全结束。
    #[cfg(feature = "experimental_changed_commit")]
    pub(crate) fn finish_prepare_and_report(
        self,
        result: &Result<MySqlPreparedTransaction, crate::MySqlPrepareReportedError>,
    ) {
        self.finish(match result {
            Ok(prepared) if prepared.registered_branch().is_some() => Outcome::PreparedRegistered,
            Ok(_) => Outcome::PreparedUnregistered,
            Err(error) => error
                .report_result()
                .map(Self::prepare_report_outcome)
                .unwrap_or(Outcome::PrepareFailureUnreported),
        });
    }
    fn finish(mut self, outcome: Outcome) {
        self.sampler.increment(self.operation, outcome);
        self.finished = true;
    }
}
impl Drop for MySqlPhaseOneGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.sampler.increment(self.operation, Outcome::Cancelled);
        }
        self.sampler.leave(self.operation);
    }
}
