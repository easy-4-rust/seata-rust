/// 独立 Undo 调用的逐字段快照；未知 COMMIT 的真实效果不计入成功观察。
/// 对应 Java: 无直接对象；WP07 独立 MySQL Undo 调用观测扩展。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MySqlUndoMetrics {
    /// 当前已轮询且尚未结算的调用数。
    pub active: u64,
    /// 原 API 明确返回 Ok 的次数，包括重复观察既有围栏。
    pub completed: u64,
    /// 原错误明确分类为本次 COMMIT 确认未知的次数。
    pub unknown_commit: u64,
    /// 本次 COMMIT 前错误次数，包含输入非法；不证明旧业务无效果。
    pub before_commit_error: u64,
    /// 已轮询 Future 取消或 panic 未返回原结果的次数，不推断提交边界。
    pub cancelled: u64,
    /// Ok(Consumed) 观察次数，包含零补偿行的已恢复或相同镜像。
    pub consumed: u64,
    /// Ok(Fenced { inserted: true }) 观察次数。
    pub fenced_inserted: u64,
    /// Ok(Fenced { inserted: false }) 观察次数，不能算作新的数据库效果。
    pub fenced_existing: u64,
    /// Ok(UndoPresent) 观察次数，不代表补偿或围栏成功。
    pub undo_present: u64,
    /// 仅累计明确 Ok(Consumed) 返回的 restored_rows；不涵盖未知 COMMIT 的真实效果。
    pub observed_restored_rows: u64,
}
