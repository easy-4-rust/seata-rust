use super::JacksonSqlUndoLog;

/// Fastjson2 原 BranchUndoLog，保留可空日志集合和原分支号。
/// 对应 Java: org.apache.seata.rm.datasource.undo.BranchUndoLog。
pub struct Fastjson2UndoBranch {
    pub(super) xid: Option<String>,
    pub(super) branch_id: i64,
    pub(super) logs: Option<Vec<JacksonSqlUndoLog>>,
}

impl Fastjson2UndoBranch {
    /// 无参数；返回可空原 XID。
    pub fn xid(&self) -> Option<&str> {
        self.xid.as_deref()
    }

    /// 无参数；返回原分支号，省略时为 0。
    pub const fn branch_id(&self) -> i64 {
        self.branch_id
    }

    /// 无参数；返回可空原日志集合的只读视图。
    pub fn logs(&self) -> Option<&[JacksonSqlUndoLog]> {
        self.logs.as_deref()
    }
}
