use crate::{Propagation, TmError};
use std::time::Duration;
/// 单次事务配置；TC事务期限与传输响应预算严格分离。
/// 对应 Java: org.apache.seata.tm.api.transaction.TransactionInfo
#[derive(Clone, PartialEq, Eq)]
pub struct TransactionConfig {
    name: String,
    timeout_millis: i32,
    propagation: Propagation,
}
/// TransactionConfig的伴随构建器，只有完整校验后才可用于Begin。
/// 对应 Java: TransactionInfo的配置组装职责。
pub struct TransactionConfigBuilder {
    name: String,
    timeout: Duration,
    propagation: Propagation,
}
impl std::fmt::Debug for TransactionConfigBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransactionConfigBuilder")
            .field("name_bytes", &self.name.len())
            .field("timeout", &self.timeout)
            .field("propagation", &self.propagation)
            .finish()
    }
}
impl TransactionConfig {
    /// 参数为原始事务名；返回默认60秒TC期限、Required策略的待校验构建器。
    pub fn builder(name: String) -> TransactionConfigBuilder {
        TransactionConfigBuilder {
            name,
            timeout: Duration::from_secs(60),
            propagation: Propagation::Required,
        }
    }
    /// 返回原样事务名。
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 返回发给TC的精确正毫秒数，不是网络等待预算。
    pub const fn timeout_millis(&self) -> i32 {
        self.timeout_millis
    }
    /// 返回嵌套传播策略。
    pub const fn propagation(&self) -> Propagation {
        self.propagation
    }
    /// 返回用于本地业务作用域计时的TC事务时长。
    pub fn transaction_timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_millis as u64)
    }
}
impl TransactionConfigBuilder {
    /// 参数是TC事务时长；小于1毫秒、非整毫秒或超出signed-i32的值在build时拒绝。
    pub fn transaction_timeout(mut self, value: Duration) -> Self {
        self.timeout = value;
        self
    }
    /// 参数为明确的传播策略；返回更新后的构建器。
    pub fn propagation(mut self, value: Propagation) -> Self {
        self.propagation = value;
        self
    }
    /// 验证名称与期限；返回不可变事务配置，不创建连接或发送请求。
    pub fn build(self) -> Result<TransactionConfig, TmError> {
        if self.name.is_empty()
            || self.name.len() > i16::MAX as usize
            || self.name.chars().any(char::is_control)
        {
            return Err(TmError::InvalidConfiguration {
                reason: "nonempty bounded transaction name without control characters required",
            });
        }
        let millis = self.timeout.as_millis();
        if millis == 0 || millis > i32::MAX as u128 || self.timeout.subsec_nanos() % 1_000_000 != 0
        {
            return Err(TmError::InvalidConfiguration {
                reason: "positive exact signed-i32 milliseconds required",
            });
        }
        Ok(TransactionConfig {
            name: self.name,
            timeout_millis: millis as i32,
            propagation: self.propagation,
        })
    }
}
impl std::fmt::Debug for TransactionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransactionConfig")
            .field("name_bytes", &self.name.len())
            .field("timeout_millis", &self.timeout_millis)
            .field("propagation", &self.propagation)
            .finish()
    }
}
