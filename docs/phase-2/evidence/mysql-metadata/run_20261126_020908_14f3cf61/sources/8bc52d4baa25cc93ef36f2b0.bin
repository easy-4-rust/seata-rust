use serde_json::Value;
use std::fmt;

/// 已检查线格式的只读对象图，保留原可空数据；尚未证明可安全恢复到数据库。
/// 对应 Java: JacksonUndoLogParser#decode的文档投影；Rust扩展，不代替事务内SqlUndoLog。
#[derive(Clone)]
pub struct UndoLogDocument {
    pub(crate) root: Value,
    pub(crate) encoded_bytes: usize,
}
impl UndoLogDocument {
    /// 无参数；返回原可空XID；不据此声明事务身份已与数据库或TC绑定。
    pub fn xid(&self) -> Option<&str> {
        self.root.get("xid").and_then(Value::as_str)
    }
    /// 无参数；返回原Java long分支号，缺失时保持Java缺省0，不限制为已登记分支。
    pub fn branch_id(&self) -> i64 {
        self.root
            .get("branchId")
            .and_then(Value::as_i64)
            .unwrap_or(0)
    }
    /// 无参数；返回原可空日志集合的元素切片，保留其中null，不据此验证Undo方向或schema。
    pub fn sql_undo_logs(&self) -> Option<&[Value]> {
        self.root
            .get("sqlUndoLogs")
            .and_then(Value::as_array)
            .and_then(|v| v.get(1))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
    }
    /// 无参数；返回已验证的不可变JSON对象图，供独立元数据绑定/差分使用。
    pub fn json(&self) -> &Value {
        &self.root
    }
}
impl PartialEq for UndoLogDocument {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root
    }
}
impl Eq for UndoLogDocument {}
impl fmt::Debug for UndoLogDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UndoLogDocument")
            .field("has_xid", &self.xid().is_some())
            .field("sql_logs", &self.sql_undo_logs().map(<[Value]>::len))
            .finish_non_exhaustive()
    }
}
