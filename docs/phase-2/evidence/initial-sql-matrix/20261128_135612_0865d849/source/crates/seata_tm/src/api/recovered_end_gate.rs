use crate::{DurableTmError, TmOperation, TransactionManager, Xid};

/// 向正式 TM 提供恢复结束发送许可的受信持久存储接口。
/// 对应 Java 无直接类型。
#[allow(async_fn_in_trait)]
pub trait RecoveredEndGate {
    /// 消耗已确认的代际申请，并在同一管理器通过状态核验、发送标记落盘后返回原 XID 和原操作。
    /// 存储实现属于信任边界，读到已置位的标记或提交结果未知时必须返回错误。
    async fn arm(self, manager: &TransactionManager) -> Result<(Xid, TmOperation), DurableTmError>;
}
