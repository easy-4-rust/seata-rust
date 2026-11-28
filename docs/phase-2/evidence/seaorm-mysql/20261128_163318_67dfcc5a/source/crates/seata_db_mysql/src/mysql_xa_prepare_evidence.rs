use crate::{mysql_xa_branch_store::MySqlXaBranchStore, DbError, MySqlDataSource};
use seata_rm::BranchIdentity;
use sqlx::{MySql, Transaction};

/// 原生产者已在 XA 内写入见证，以及原 Prepared 已进入持久回滚结束意图的证据。
/// 对应 Java: 无直接对象；Rust XA 生产者与 Prepared 回滚恢复扩展。
pub(crate) struct MySqlXaPrepareEvidence;
impl MySqlXaPrepareEvidence {
    pub(crate) const TABLE: &'static str = "seata_rs_xa_prepare_evidence_v1";

    /// 参数为持原门闩的控制事务、原数据源和身份；唯一插入已写见证证据，提交由调用者确认。
    pub(crate) async fn stage(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        id: &BranchIdentity,
    ) -> Result<(), DbError> {
        MySqlXaBranchStore::validate_table(tx, source, Self::TABLE).await?;
        let changed = sqlx::query("INSERT INTO seata_rs_xa_prepare_evidence_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,?,?,1,0,0)")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes())
            .bind(id.application_data().map(str::as_bytes)).bind(source.server_uuid.as_bytes())
            .execute(&mut **tx).await.map_err(|e| DbError::driver("XA staged evidence insert", e))?;
        if changed.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }

    /// 参数为持原门闩事务和原身份；仅调用方已实际观察 Prepared 时使用，返回是否必须先提交新意图。
    /// None 表示无原生产者证据的旧分支，仅可依靠本次原生响应，不据缺行恢复终态。
    pub(crate) async fn arm_rollback(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        id: &BranchIdentity,
    ) -> Result<Option<bool>, DbError> {
        let record = MySqlXaBranchStore::read_table(tx, source, id, Self::TABLE).await?;
        let Some(record) = record else {
            return Ok(None);
        };
        if record.completed || !matches!(record.phase, 0 | 2) {
            return Err(DbError::RecoveryConflict);
        }
        if record.phase == 2 {
            return Ok(Some(false));
        }
        let changed = sqlx::query("UPDATE seata_rs_xa_prepare_evidence_v1 SET phase=2 WHERE xid=? AND branch_id=? AND phase=0 AND completed=0")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).execute(&mut **tx).await.map_err(|e| DbError::driver("XA Prepared rollback intent", e))?;
        if changed.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(Some(true))
    }

    /// 参数为持原门闩事务、数据源和身份；只返回精确匹配且已经持久观察 Prepared 的原回滚意图。
    pub(crate) async fn rollback_armed(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        id: &BranchIdentity,
    ) -> Result<bool, DbError> {
        let record = MySqlXaBranchStore::read_table(tx, source, id, Self::TABLE).await?;
        match record {
            Some(record) if record.completed || !matches!(record.phase, 0 | 2) => {
                Err(DbError::RecoveryConflict)
            }
            Some(record) => Ok(record.phase == 2),
            None => Ok(false),
        }
    }
}
