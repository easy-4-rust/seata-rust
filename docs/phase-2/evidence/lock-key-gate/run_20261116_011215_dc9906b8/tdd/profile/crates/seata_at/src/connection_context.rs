use crate::{BaseTransactionalExecutor, LockKeyError, MySqlJdbcProfile, SqlType, SqlUndoLog};
use std::{collections::BTreeSet, fmt};

/// 有界的原逐语句锁键集合；不实现原保存点、XID绑定或分支状态，不能当作完整事务上下文。
/// 对应 Java: org.apache.seata.rm.datasource.ConnectionContext（锁集合部分实现）。
pub struct ConnectionContext {
    lock_keys: BTreeSet<String>,
    total_bytes: usize,
    maximum_bytes: usize,
    undo_count: usize,
}
impl ConnectionContext {
    /// 无参数；返回空锁集合，累计输出硬上限1MiB。
    pub fn new() -> Self {
        Self::default()
    }
    /// 参数为累计UTF-8字节上限；只允许1..1MiB，返回独立空集合。
    pub fn with_limit(maximum_bytes: usize) -> Result<Self, LockKeyError> {
        if !(1..=1_048_576).contains(&maximum_bytes) {
            return Err(LockKeyError::Invalid {
                reason: "lock set budget",
            });
        }
        Ok(Self {
            maximum_bytes,
            ..Self::default()
        })
    }
    /// 参数为原准备日志和JDBC配置；返回锁集合，任何失败不产生半个结果。
    pub fn from_undo_logs(
        logs: &[SqlUndoLog],
        profile: MySqlJdbcProfile,
    ) -> Result<Self, LockKeyError> {
        if logs.len() > 128 {
            return Err(LockKeyError::Budget {
                limit: "128 lock source logs",
            });
        }
        let mut context = Self::new();
        for log in logs {
            context.append_undo(log, profile)?;
        }
        Ok(context)
    }
    /// 参数为已检查的SQL日志及明确JDBC配置；按DELETE前镜像/其他后镜像追加，失败保持原集合。
    /// 对应 Java: BaseTransactionalExecutor#prepareUndoLog及ConnectionContext#appendLockKey。
    pub fn append_undo(
        &mut self,
        log: &SqlUndoLog,
        profile: MySqlJdbcProfile,
    ) -> Result<(), LockKeyError> {
        if self.undo_count >= 128 {
            return Err(LockKeyError::Budget {
                limit: "128 lock source logs",
            });
        }
        let records = if log.sql_type() == SqlType::Delete {
            log.before_image()
        } else {
            log.after_image()
        };
        let key = BaseTransactionalExecutor::build_lock_key(records, profile)?;
        if let Some(key) = key {
            if !self.lock_keys.contains(&key) {
                let separator = usize::from(!self.lock_keys.is_empty());
                let size = self
                    .total_bytes
                    .checked_add(key.len())
                    .and_then(|size| size.checked_add(separator))
                    .filter(|size| *size <= self.maximum_bytes)
                    .ok_or(LockKeyError::Budget {
                        limit: "aggregate lock key bytes",
                    })?;
                self.lock_keys.insert(key);
                self.total_bytes = size;
            }
        }
        self.undo_count += 1;
        Ok(())
    }
    /// 无参数；返回分号连接的稳定完整键集合，空集合为None，不改变各语句行序。
    /// 对应 Java: ConnectionContext#buildLockKeys；Java HashSet顺序不作为兼容比较目标。
    pub fn build_lock_keys(&self) -> Option<String> {
        if self.lock_keys.is_empty() {
            return None;
        }
        let mut result = String::with_capacity(self.total_bytes);
        for (index, key) in self.lock_keys.iter().enumerate() {
            if index != 0 {
                result.push(';');
            }
            result.push_str(key);
        }
        Some(result)
    }
    /// 无参数；返回去重后的完整语句锁键数量，不是TC解析出的行锁数量。
    pub fn lock_key_count(&self) -> usize {
        self.lock_keys.len()
    }
}
impl Default for ConnectionContext {
    fn default() -> Self {
        Self {
            lock_keys: BTreeSet::new(),
            total_bytes: 0,
            maximum_bytes: 1_048_576,
            undo_count: 0,
        }
    }
}
impl fmt::Debug for ConnectionContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectionContext")
            .field("lock_keys", &self.lock_keys.len())
            .field("bytes", &self.total_bytes)
            .finish_non_exhaustive()
    }
}
