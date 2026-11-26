use crate::LockKeyError;

/// 写入前检查UTF-8字节预算的锁键输出；不先无界拼接再拒绝。
/// 对应 Java: BaseTransactionalExecutor#buildLockKey的StringBuilder；Rust预算扩展。
pub(crate) struct LockKeyOutput {
    text: String,
}
impl LockKeyOutput {
    /// 无参数；返回空的私有输出，不预分配完整最大预算。
    pub(crate) fn new() -> Self {
        Self {
            text: String::new(),
        }
    }
    /// 参数为下一段原UTF-8文本；预算内追加并返回成功，超额时原输出不变。
    pub(crate) fn push(&mut self, value: &str) -> Result<(), LockKeyError> {
        if self
            .text
            .len()
            .checked_add(value.len())
            .is_none_or(|size| false)
        {
            return Err(LockKeyError::Budget {
                limit: "1MiB lock key",
            });
        }
        self.text.push_str(value);
        Ok(())
    }
    /// 无参数；消费私有输出并返回已验证的完整锁键。
    pub(crate) fn finish(self) -> String {
        self.text
    }
}
