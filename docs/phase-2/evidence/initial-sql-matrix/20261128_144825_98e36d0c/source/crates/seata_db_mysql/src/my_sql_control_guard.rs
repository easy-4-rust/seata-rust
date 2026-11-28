use crate::{MySqlRecoveryError, MySqlRecoveryMetricsSampler, MySqlRecoveryOperation};
use seata_rm::BranchResolution;

/// 结算内部控制调用，取消不伪造提交边界，读取 Applied 不计业务执行次数。
/// 对应 Java: 无直接对象；WP07 控制事务作用域计数扩展。
pub(crate) struct MySqlControlGuard {
    sampler: MySqlRecoveryMetricsSampler,
    operation: MySqlRecoveryOperation,
    finished: bool,
}
impl MySqlControlGuard {
    /// 参数为内部采样能力和实际动作；返回一份独占生命周期守卫。
    pub(crate) fn new(
        sampler: MySqlRecoveryMetricsSampler,
        operation: MySqlRecoveryOperation,
    ) -> Self {
        Self {
            sampler,
            operation,
            finished: false,
        }
    }
    /// 参数为原样控制结果引用；无返回值，不修改错误或控制决定。
    pub(crate) fn finish(mut self, result: &Result<BranchResolution, MySqlRecoveryError>) {
        let field = match result {
            Ok(_) => 1,
            Err(error) if error.is_unknown_outcome() => 2,
            Err(_) => 3,
        };
        self.sampler.increment(self.operation, field);
        // 只有现有 control 真正返回的两种成功结果才有附加分类。
        match result {
            Ok(BranchResolution::Applied) => self.sampler.increment(self.operation, 5),
            Ok(BranchResolution::Unknown) => self.sampler.increment(self.operation, 6),
            _ => {}
        }
        self.finished = true;
    }
}
impl Drop for MySqlControlGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.sampler.increment(self.operation, 4);
        }
        self.sampler.leave(self.operation);
    }
}
