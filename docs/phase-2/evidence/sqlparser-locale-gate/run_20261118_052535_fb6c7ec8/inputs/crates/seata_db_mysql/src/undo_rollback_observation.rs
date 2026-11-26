/// 原Normal日志处理/独立控制提交的事实，不直接充当通用RM执行历史。
/// 对应 Java: AbstractUndoLogManager#undo的原Normal/GlobalFinished分支（Rust结果扩展）。
#[derive(Debug, PartialEq, Eq)]
pub enum UndoRollbackObservation {
    /// 本次原日志已消费并明确提交；不表示其他旧尝试都已围栏。
    Consumed {
        /// 逆序验证/处理的原日志数，包含已恢复或无变化的日志。
        logs: usize,
        /// 本次实际执行的逐行补偿DML更新数。
        restored_rows: u64,
    },
    /// 无Normal需处理，已确认原GlobalFinished围栏；不证明业务历史。
    Fenced {
        /// 本次是否新建并明确提交控制记录。
        inserted: bool,
    },
}
