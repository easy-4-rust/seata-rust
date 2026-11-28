use super::RmMetricsSampler;
use crate::{BranchPhase, BranchRecordState, BranchResolution, RmError};

/// 一次分派作用域的计数守卫；取消不制造完成或安全重试事实。
/// 对应 Java: 无直接对象；WP07 RM 分派生命周期观测扩展。
pub(crate) struct RmDispatchGuard {
    sampler: RmMetricsSampler,
    phase: BranchPhase,
    outcome: Option<usize>,
    finished: bool,
}

impl RmDispatchGuard {
    /// 参数为内部计数能力及原方向；返回尚未结算的守卫。
    pub(crate) fn new(sampler: RmMetricsSampler, phase: BranchPhase) -> Self {
        Self {
            sampler,
            phase,
            outcome: None,
            finished: false,
        }
    }
    /// 参数为已同步持久终态；无返回值，仅标记结果复用，不计新的资源执行。
    pub(crate) fn reused(&mut self, state: BranchRecordState) {
        self.outcome = Some(match state {
            BranchRecordState::Completed => 10,
            BranchRecordState::Rejected => 11,
            _ => 12,
        });
    }
    /// 参数为资源结果；无返回值，调用方必须先完成相应持久日志写入。
    pub(crate) fn resolved(&mut self, resolution: BranchResolution) {
        self.outcome = Some(match resolution {
            BranchResolution::Applied => 5,
            BranchResolution::Rejected => 6,
            BranchResolution::NotApplied => 7,
            BranchResolution::Unknown => 8,
            BranchResolution::ManualInterventionRequired => 9,
        });
    }
    /// 无参数、无返回值；在进入资源 apply 调用前记录一次尝试。
    pub(crate) fn apply(&self) {
        self.sampler.increment(self.phase, 14);
    }
    /// 无参数、无返回值；在进入资源 recover 调用前记录一次尝试。
    pub(crate) fn recover(&self) {
        self.sampler.increment(self.phase, 15);
    }
    /// 参数为真实分派返回结果；消费守卫并计数，无返回值。
    pub(crate) fn finish<T>(mut self, result: &Result<T, RmError>) {
        let field = if result.is_err() {
            12
        } else {
            self.outcome.unwrap_or(12)
        };
        self.sampler.increment(self.phase, field);
        self.finished = true;
    }
}
impl Drop for RmDispatchGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.sampler.increment(self.phase, 13);
        }
        self.sampler.leave(self.phase);
    }
}
