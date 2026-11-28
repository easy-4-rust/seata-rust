use crate::TccError;
/// 原 Java Fence 的持久状态码，不使用 Rust 枚举序号作为协议或数据库值。
/// 对应 Java: org.apache.seata.integration.tx.api.fence.constant.CommonFenceConstant。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonFenceStatus {
    /// Try 与 Fence 同事务已执行。
    Tried,
    /// Confirm 与 Fence 同事务已执行。
    Committed,
    /// Cancel 与 Fence 同事务已执行。
    Rollbacked,
    /// 空 Cancel 的持久屏障，拒绝迟到 Try。
    Suspended,
}
impl CommonFenceStatus {
    /// 无参数；返回原 Java 固定持久码 1、2、3 或 4。
    pub const fn code(self) -> i32 {
        match self {
            Self::Tried => 1,
            Self::Committed => 2,
            Self::Rollbacked => 3,
            Self::Suspended => 4,
        }
    }
}
impl TryFrom<i32> for CommonFenceStatus {
    type Error = TccError;
    fn try_from(code: i32) -> Result<Self, Self::Error> {
        match code {
            1 => Ok(Self::Tried),
            2 => Ok(Self::Committed),
            3 => Ok(Self::Rollbacked),
            4 => Ok(Self::Suspended),
            _ => Err(TccError::UnknownFenceStatus),
        }
    }
}
