use crate::LockKeyError;

/// 写入前检查UTF-8字节预算的锁键输出；不先无界拼接再拒绝。
/// 对应 Java: BaseTransactionalExecutor#buildLockKey的StringBuilder；Rust预算扩展。
pub(crate) struct LockKeyOutput {
    text: String,
}
impl LockKeyOutput {
    pub(crate) fn new() -> Self {
        Self {
            text: String::new(),
        }
    }
    pub(crate) fn push(&mut self, value: &str) -> Result<(), LockKeyError> {
        if self
            .text
            .len()
            .checked_add(value.len())
            .is_none_or(|size| size > 1_048_576)
        {
            return Err(LockKeyError::Budget {
                limit: "1MiB lock key",
            });
        }
        self.text.push_str(value);
        Ok(())
    }
    pub(crate) fn finish(self) -> String {
        self.text
    }
}
