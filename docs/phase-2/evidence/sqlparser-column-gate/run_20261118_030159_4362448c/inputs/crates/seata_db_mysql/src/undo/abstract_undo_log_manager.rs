use super::AbstractUndoExecutor;
use crate::{
    cache::MySqlTableMetaReader,
    mysql::{MySqlUndoLogManager, UndoFenceDecision},
    DbError, UndoRollbackObservation,
};
use seata_at::{JacksonUndoLogParser, MySqlJdbcProfile};
use sqlx::{MySql, Transaction};

/// 原Normal反序补偿与同事务清理；结果为待提交候选，调用者负责控制COMMIT/未知分类。
/// 对应 Java: org.apache.seata.rm.datasource.undo.AbstractUndoLogManager#undo（既有MySQL类型范围）。
pub(crate) struct AbstractUndoLogManager;
impl AbstractUndoLogManager {
    /// 参数为独占控制事务/schema/原身份/JDBC配置；返回待完成结果与是否需要控制COMMIT，禁止提前发布。
    pub(crate) async fn prepare(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        xid: &str,
        branch_id: i64,
        profile: MySqlJdbcProfile,
    ) -> Result<(UndoRollbackObservation, bool), DbError> {
        match MySqlUndoLogManager::prepare_fence(tx, database, xid, branch_id).await? {
            UndoFenceDecision::InsertedGuard => {
                return Ok((UndoRollbackObservation::Fenced { inserted: true }, true))
            }
            UndoFenceDecision::ExistingGuard => {
                return Ok((UndoRollbackObservation::Fenced { inserted: false }, false))
            }
            UndoFenceDecision::NormalRow => {}
        }
        let (bytes, ids) = MySqlUndoLogManager::read_normal(tx, xid, branch_id).await?;
        let parser = JacksonUndoLogParser::default();
        let doc = parser.decode(&bytes)?.ok_or(DbError::Input {
            reason: "Undo non-null original document",
        })?;
        if doc.xid() != Some(xid) || doc.branch_id() != branch_id {
            return Err(DbError::Input {
                reason: "Undo original document identity",
            });
        }
        let raw = doc.sql_undo_logs().ok_or(DbError::Input {
            reason: "Undo concrete SQL logs",
        })?;
        let mut tables = Vec::new();
        for log in raw {
            let name = log
                .get("tableName")
                .and_then(|value| value.as_str())
                .ok_or(DbError::Input {
                    reason: "Undo concrete table name",
                })?;
            if !tables
                .iter()
                .any(|table: &seata_datasource::TableMeta| table.name() == name)
            {
                tables.push(MySqlTableMetaReader::fetch(tx, database, name).await?);
            }
        }
        // 全部身份/元数据/方向/类型和值域通过后才允许第一条补偿DML；后续错误仍整体回滚。
        let logs = parser.restore_prepared(&doc, xid, branch_id, &tables, profile)?;
        let mut restored_rows = 0;
        for log in logs.iter().rev() {
            restored_rows += AbstractUndoExecutor::execute(tx, database, log).await?;
        }
        MySqlUndoLogManager::delete_normal(tx, xid, branch_id, &ids).await?;
        Ok((
            UndoRollbackObservation::Consumed {
                logs: logs.len(),
                restored_rows,
            },
            true,
        ))
    }
}
