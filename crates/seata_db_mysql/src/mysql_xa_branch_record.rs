/// 已核对不可变原身份的 XA 控制行快照，不构造 RegisteredBranch 或推导缺失效果。
/// 对应 Java: 无直接对象；Rust XA 持久方向/终态扩展。
#[derive(Debug, Clone, Copy)]
pub(crate) struct MySqlXaBranchRecord {
    /// 0 未绑定，1 Commit，2 Rollback。
    pub(crate) phase: u8,
    /// 原生结束响应及此终态控制 COMMIT 均已确认。
    pub(crate) completed: bool,
}
