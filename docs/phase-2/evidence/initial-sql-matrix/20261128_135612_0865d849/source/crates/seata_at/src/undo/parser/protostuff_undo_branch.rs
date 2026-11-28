use super::JacksonSqlUndoLog;

/// Protostuff 原分支对象，保留可空日志列表。
/// 对应 Java: org.apache.seata.rm.datasource.undo.BranchUndoLog。
pub struct ProtostuffUndoBranch {
    pub(super) xid: Option<String>,
    pub(super) branch_id: i64,
    pub(super) logs: Option<Vec<JacksonSqlUndoLog>>,
}

impl ProtostuffUndoBranch {
    /// 无参数；返回原可空 XID。
    pub fn xid(&self) -> Option<&str> {
        self.xid.as_deref()
    }

    /// 无参数；返回原分支号，未写字段时返回零。
    pub const fn branch_id(&self) -> i64 {
        self.branch_id
    }

    /// 无参数；返回原日志列表，无重复字段时返回 None。
    pub fn logs(&self) -> Option<&[JacksonSqlUndoLog]> {
        self.logs.as_deref()
    }
}
