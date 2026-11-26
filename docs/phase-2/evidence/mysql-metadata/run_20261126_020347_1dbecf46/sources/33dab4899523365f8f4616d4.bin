use super::{AbstractUndoExecutor, RollbackUndoDocument, UndoSerializer};
use crate::{
    cache::MySqlTableMetaReader,
    mysql::{MySqlUndoLogManager, UndoFenceDecision},
    DbError, UndoRollbackObservation,
};
use seata_at::MySqlJdbcProfile;
use sqlx::{mysql::MySqlConnectOptions, MySql, Transaction};

/// 原Normal反序补偿与同事务清理；结果为待提交候选，调用者负责控制COMMIT/未知分类。
/// 对应 Java: org.apache.seata.rm.datasource.undo.AbstractUndoLogManager#undo（既有MySQL类型范围）。
pub(crate) struct AbstractUndoLogManager;
impl AbstractUndoLogManager {
    /// 参数为独占控制事务/原连接配置/schema/原身份/JDBC配置；返回待完成结果与是否需要控制COMMIT，禁止提前发布。
    pub(crate) async fn prepare(
        tx: &mut Transaction<'static, MySql>,
        options: &MySqlConnectOptions,
        database: &str,
        xid: &str,
        branch_id: i64,
        profile: MySqlJdbcProfile,
    ) -> Result<(UndoRollbackObservation, bool), DbError> {
        Self::prepare_for_serializer(
            tx,
            options,
            database,
            xid,
            branch_id,
            profile,
            UndoSerializer::Jackson,
        )
        .await
    }

    /// 参数为原事务/连接配置/schema/原身份/JDBC配置/格式；返回候选结果与是否需要控制提交。
    /// 按调用方要求并经锁定根行核对的格式执行同一反序补偿及清理流程。
    /// 对应 Java: AbstractUndoLogManager#undo。
    pub(crate) async fn prepare_for_serializer(
        tx: &mut Transaction<'static, MySql>,
        options: &MySqlConnectOptions,
        database: &str,
        xid: &str,
        branch_id: i64,
        profile: MySqlJdbcProfile,
        format: UndoSerializer,
    ) -> Result<(UndoRollbackObservation, bool), DbError> {
        let fence =
            MySqlUndoLogManager::prepare_fence(tx, options, database, xid, branch_id).await?;
        if matches!(
            fence,
            UndoFenceDecision::InsertedGuard | UndoFenceDecision::ExistingGuard
        ) {
            // 残缺的分片不能证明分支从未写入 Undo；新建围栏也必须随失败回滚。
            MySqlUndoLogManager::require_no_rollback_children(tx, xid, branch_id).await?;
            let inserted = matches!(fence, UndoFenceDecision::InsertedGuard);
            return Ok((UndoRollbackObservation::Fenced { inserted }, inserted));
        }
        let (bytes, ids) =
            MySqlUndoLogManager::read_normal_for_serializer(tx, xid, branch_id, format).await?;
        let document = RollbackUndoDocument::decode(format, &bytes, xid, branch_id)?;
        let mut tables = Vec::new();
        for name in document.table_names()? {
            // Undo 字典可能先于业务表锁建立旧快照；在持锁期间另用同身份新会话核对当前约束。
            crate::current_table_constraints::CurrentTableConstraints::verify(
                tx, options, database, name,
            )
            .await?;
            tables.push(MySqlTableMetaReader::fetch(tx, database, name).await?);
        }
        // 全部身份/元数据/方向/类型和值域通过后才允许第一条补偿DML；后续错误仍整体回滚。
        let logs = document.bind(xid, branch_id, &tables, profile)?;
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
