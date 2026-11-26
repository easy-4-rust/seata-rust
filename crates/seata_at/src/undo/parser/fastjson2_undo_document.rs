use super::{Fastjson2UndoBranch, Fastjson2UndoLogParser, MySqlJdbcProfile, UndoCodecError};
use crate::SqlUndoLog;
use seata_datasource::TableMeta;

/// 使用原 JSONB 预算完成预检的只读文档，用于同一回滚事务的表查询与物理绑定。
/// 对应 Java: AbstractUndoLogManager#undo 解码后的 BranchUndoLog（Rust 预检扩展）。
pub struct Fastjson2UndoDocument {
    pub(super) branch: Fastjson2UndoBranch,
    pub(super) tables: Vec<String>,
}

impl Fastjson2UndoDocument {
    /// 无参数；返回按首次出现顺序去重的已验证表名。
    pub fn table_names(&self) -> impl Iterator<Item = &str> {
        self.tables.iter().map(String::as_str)
    }

    /// 参数为锁定行的事务身份、当前完整物理元数据及 JDBC 配置，返回同一文档的有序回滚镜像。
    /// 对应 Java: AbstractUndoLogManager#undo 中的 SQLUndoLog#setTableMeta。
    pub fn bind(
        &self,
        xid: &str,
        branch_id: i64,
        tables: &[TableMeta],
        profile: MySqlJdbcProfile,
    ) -> Result<Vec<SqlUndoLog>, UndoCodecError> {
        if self.branch.xid() != Some(xid) || self.branch.branch_id() != branch_id {
            return Err(UndoCodecError::Shape {
                reason: "JSONB restored original branch identity",
            });
        }
        let logs = self.branch.logs().ok_or(UndoCodecError::Shape {
            reason: "JSONB restored nonempty log collection",
        })?;
        Fastjson2UndoLogParser::bind_logs(logs, tables, profile)
    }
}
