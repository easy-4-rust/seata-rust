use crate::TransportError;

/// 单连接独占的有符号请求ID分配器；编号耗尽要求连接轮换，同一连接永不回收旧编号。
/// 对应 Java: org.apache.seata.core.rpc.netty.AbstractNettyRemoting#getNextMessageId 的发号职责；新增Rust连接代际安全约束。
/// 不实现Clone，不能复制计数状态供同一连接并行分配；可由连接actor或外部互斥所有权串行调用。
#[derive(Debug)]
pub struct RequestIdAllocator {
    next_id: i32,
    issued: u64,
    maximum: u64,
}
impl RequestIdAllocator {
    /// 参数first_id为第一个返回的Java signed-int编号，maximum为该连接的发号额度1..=2^32。
    /// 返回唯一所有权分配器；非法额度返回配置错误，不限制负ID或零值。
    pub fn new(first_id: i32, maximum: u64) -> Result<Self, TransportError> {
        if maximum == 0 || maximum > (1u64 << 32) {
            return Err(TransportError::InvalidConfiguration {
                reason: "per-connection ID quota must be in 1..=2^32",
            });
        }
        Ok(Self {
            next_id: first_id,
            issued: 0,
            maximum,
        })
    }
    /// 返回本连接下一唯一编号；按Java int位模式环绕，额度用尽后只返回连接轮换错误。
    /// 完成、取消、超时或未发送失败都不能让调用方在本连接复用已发出的编号。
    pub fn allocate(&mut self) -> Result<i32, TransportError> {
        if self.issued == self.maximum {
            return Err(TransportError::RequestIdRotationRequired {
                issued: self.issued,
                maximum: self.maximum,
            });
        }
        let value = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.issued += 1;
        Ok(value)
    }
    /// 返回累计已发号数；不表示当前in-flight数量。
    pub const fn issued(&self) -> u64 {
        self.issued
    }
    /// 返回本连接轮换前剩余的发号额度。
    pub const fn remaining(&self) -> u64 {
        self.maximum - self.issued
    }
}

#[cfg(test)]
mod tests {
    use super::RequestIdAllocator;

    #[test]
    fn final_two_ids_of_a_valid_full_space_epoch_exhaust_without_reuse() {
        // 模拟同一有效序列已完成2^32-2次发号的状态，仅实际执行最后两次；不声称循环了四十亿次。
        let mut ids = RequestIdAllocator {
            next_id: -1,
            issued: (1u64 << 32) - 2,
            maximum: 1u64 << 32,
        };
        assert_eq!(ids.allocate().ok(), Some(-1));
        assert_eq!(ids.allocate().ok(), Some(0));
        assert!(ids.allocate().is_err());
        assert_eq!(ids.remaining(), 0);
    }
}
