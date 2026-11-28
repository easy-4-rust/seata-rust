/// 独立 Undo API 的固定调用类别，不代表恢复管理器控制事务或业务效果。
/// 对应 Java: 无直接对象；WP07 独立 MySQL Undo 调用观测扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MySqlUndoOperation {
    /// 原身份独立补偿或防悬挂观察。
    Rollback,
    /// 原身份独立围栏或 Normal 存在性观察。
    Fence,
}
