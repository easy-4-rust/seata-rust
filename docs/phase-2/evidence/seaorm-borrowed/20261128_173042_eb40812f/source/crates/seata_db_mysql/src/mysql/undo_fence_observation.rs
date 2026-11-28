/// 原Undo唯一身份的控制观察；不把存在性或围栏视为业务历史/二阶段执行证明。
/// 对应 Java: AbstractUndoLogManager#undo的缺失日志防悬挂分支（Rust显式结果扩展）。
#[derive(Debug, PartialEq, Eq)]
pub enum UndoFenceObservation {
    /// 已确认原GlobalFinished围栏；inserted表示本次控制事务是否新建该行。
    Fenced {
        /// true仅用于本次明确完成控制COMMIT；false表示确认此前已提交的完整guard。
        inserted: bool,
    },
    /// 原Normal记录存在，需要后续读取/补偿；不返回其可能很大的未认证内容。
    UndoPresent,
}
