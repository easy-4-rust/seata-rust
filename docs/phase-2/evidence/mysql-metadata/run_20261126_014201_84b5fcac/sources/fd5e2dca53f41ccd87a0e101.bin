use super::JacksonSqlUndoLog;

/// 固定 Kryo 工厂的原分支对象。
/// 对应 Java: org.apache.seata.rm.datasource.undo.BranchUndoLog。
pub struct KryoUndoBranch {
    pub(super) xid: Option<String>,
    pub(super) branch_id: i64,
    pub(super) logs: Option<Vec<JacksonSqlUndoLog>>,
}

impl KryoUndoBranch {
    /// 无参数；返回原可空 XID。
    pub fn xid(&self) -> Option<&str> {
        self.xid.as_deref()
    }

    /// 无参数；返回原 Java long 分支号。
    pub const fn branch_id(&self) -> i64 {
        self.branch_id
    }

    /// 无参数；返回可空的原 SQL 日志集合。
    pub fn logs(&self) -> Option<&[JacksonSqlUndoLog]> {
        self.logs.as_deref()
    }
}
