use crate::TransactionCompletion;
/// 业务成功值与独立全局完成观察；不能仅凭业务值推断TC已物理清理。
/// 对应 Java: TransactionalTemplate返回值及DefaultGlobalTransaction状态组合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionResult<T> {
    pub(crate) value: T,
    pub(crate) completion: TransactionCompletion,
}
impl<T> TransactionResult<T> {
    /// 返回业务成功值借用。
    pub const fn value(&self) -> &T {
        &self.value
    }
    /// 返回真实TC观察、参与方或无事务完成标记。
    pub const fn completion(&self) -> &TransactionCompletion {
        &self.completion
    }
    /// 消费结果，返回业务值与全局完成观察，不丢弃任一部分。
    pub fn into_parts(self) -> (T, TransactionCompletion) {
        (self.value, self.completion)
    }
}
