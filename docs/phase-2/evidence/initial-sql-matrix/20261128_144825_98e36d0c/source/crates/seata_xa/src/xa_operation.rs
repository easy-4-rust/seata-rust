/// XA 原生调用的固定阶段，不包含 SQL 或业务身份。
/// 对应 Java: XAResource 的 start/end/prepare/commit/rollback/recover 操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XaOperation {
    /// 原身份开启。
    Start,
    /// 原单条业务 DML。
    Execute,
    /// 原 END 及 PREPARE。
    Prepare,
    /// 显式原身份提交。
    Commit,
    /// 显式原身份回滚。
    Rollback,
    /// 只读 Prepared 观察。
    Recover,
}
