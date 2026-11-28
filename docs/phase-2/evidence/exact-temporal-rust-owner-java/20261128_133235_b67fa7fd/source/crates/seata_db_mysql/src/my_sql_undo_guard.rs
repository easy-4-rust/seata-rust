use crate::{
    MySqlUndoMetricsSampler, MySqlUndoOperation, UndoFenceError, UndoFenceObservation,
    UndoRollbackError, UndoRollbackObservation,
};

/// 独立 Undo 调用的独占结算守卫，不改变原结果、异常或事务状态。
/// 对应 Java: 无直接对象；WP07 独立 MySQL Undo 调用观测扩展。
pub(crate) struct MySqlUndoGuard {
    sampler: MySqlUndoMetricsSampler,
    operation: MySqlUndoOperation,
    finished: bool,
}
impl MySqlUndoGuard {
    /// 参数为内部采样能力及固定类别；返回一份独占守卫。
    pub(crate) fn new(sampler: MySqlUndoMetricsSampler, operation: MySqlUndoOperation) -> Self {
        Self {
            sampler,
            operation,
            finished: false,
        }
    }
    /// 参数为原样补偿结果引用；无返回值，仅成功观察携带行数，不统计未知提交的效果。
    pub(crate) fn finish_rollback(
        mut self,
        result: &Result<UndoRollbackObservation, UndoRollbackError>,
    ) {
        let field = match result {
            Ok(_) => 1,
            Err(e) if e.is_unknown_outcome() => 2,
            Err(_) => 3,
        };
        self.sampler.add(self.operation, field, 1);
        match result {
            Ok(UndoRollbackObservation::Consumed { restored_rows, .. }) => {
                self.sampler.add(self.operation, 5, 1);
                self.sampler.add(self.operation, 9, *restored_rows);
            }
            Ok(UndoRollbackObservation::Fenced { inserted }) => {
                self.sampler
                    .add(self.operation, if *inserted { 6 } else { 7 }, 1)
            }
            Err(_) => {}
        }
        self.finished = true;
    }
    /// 参数为原样围栏结果引用；无返回值，存在 Normal 或既有 guard 仅计观察。
    pub(crate) fn finish_fence(mut self, result: &Result<UndoFenceObservation, UndoFenceError>) {
        let field = match result {
            Ok(_) => 1,
            Err(e) if e.is_unknown_outcome() => 2,
            Err(_) => 3,
        };
        self.sampler.add(self.operation, field, 1);
        match result {
            Ok(UndoFenceObservation::Fenced { inserted }) => {
                self.sampler
                    .add(self.operation, if *inserted { 6 } else { 7 }, 1)
            }
            Ok(UndoFenceObservation::UndoPresent) => self.sampler.add(self.operation, 8, 1),
            Err(_) => {}
        }
        self.finished = true;
    }
}
impl Drop for MySqlUndoGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.sampler.add(self.operation, 4, 1);
        }
        self.sampler.leave(self.operation);
    }
}
