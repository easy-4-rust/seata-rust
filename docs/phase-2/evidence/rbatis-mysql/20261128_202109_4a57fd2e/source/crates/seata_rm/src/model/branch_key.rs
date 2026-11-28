use std::fmt;

/// 分支去重键；资源及方向作为不可变记录校验，不能通过换资源绕过去重。
/// 对应 Java: BranchSession的xid/branchId组合；Rust持久协调扩展。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BranchKey {
    pub(crate) xid: String,
    pub(crate) branch_id: i64,
}
impl fmt::Debug for BranchKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BranchKey")
            .field("xid_bytes", &self.xid.len())
            .field("branch_id", &self.branch_id)
            .finish()
    }
}
