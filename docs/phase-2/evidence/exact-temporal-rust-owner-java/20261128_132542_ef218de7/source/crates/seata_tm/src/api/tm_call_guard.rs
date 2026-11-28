use super::TmMetricsSampler;
use crate::{TmError, TmOperation};

/// 维护一次 TM 原生调用的计数生命周期；未结算退出只归类为 cancelled。
/// 对应 Java: 无直接对象；WP07 作用域指标守卫扩展。
pub(crate) struct TmCallGuard {
    sampler: TmMetricsSampler,
    operation: TmOperation,
    finished: bool,
}

impl TmCallGuard {
    /// 参数为只读源的内部计数能力和原操作；返回尚未结算的调用守卫。
    pub(crate) fn new(sampler: TmMetricsSampler, operation: TmOperation) -> Self {
        Self {
            sampler,
            operation,
            finished: false,
        }
    }

    /// 参数为原生调用的真实结果引用；消费守卫、归类并释放 active，无返回值。
    pub(crate) fn finish<T>(mut self, result: &Result<T, TmError>) {
        // 复用正式错误语义，避免把写后协议失败或 TC Failed 当成成功。
        let field = match result {
            Ok(_) => 1,
            Err(TmError::DefiniteRejected { .. }) => 2,
            Err(error) if error.is_unknown_outcome() => 3,
            Err(TmError::LocalFailureBeforeSend { .. } | TmError::ProtocolFailure { .. }) => 4,
            Err(_) => 5,
        };
        self.sampler.increment(self.operation, field);
        if matches!(result, Err(TmError::ProtocolFailure { .. })) {
            self.sampler.increment(self.operation, 7);
        }
        self.finished = true;
    }
}

impl Drop for TmCallGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.sampler.increment(self.operation, 6);
        }
        self.sampler.leave(self.operation);
    }
}
