use crate::{
    mysql_xa_branch_store::MySqlXaBranchStore, mysql_xa_commit_witness::MySqlXaCommitWitness,
    mysql_xa_prepare_evidence::MySqlXaPrepareEvidence, mysql_xa_session_fence::MySqlXaSessionFence,
    DbError, MySqlXaDataSource,
};
use seata_rm::{BranchCallbackReceipt, BranchPhase};

/// 正式早回滚回调的启动否定记录；只竞争唯一键，不签发或补造 RegisteredBranch。
/// 对应 Java: 无直接对象；Rust XA 回调先于登记响应的持久交接扩展。
pub(crate) struct MySqlXaEarlyRollback;
impl MySqlXaEarlyRollback {
    /// 参数为原数据源和正式回调身份；仅在无原效果且赢得唯一登记键时原子持久否定终态与 guard。
    /// 已有原登记或并发登记获胜时不作完成推断，交回既有执行/恢复路径。
    pub(crate) async fn observe(
        native: &MySqlXaDataSource,
        receipt: &BranchCallbackReceipt,
    ) -> Result<(), DbError> {
        if receipt.phase() != BranchPhase::Rollback {
            return Err(DbError::RecoveryConflict);
        }
        let id = receipt.identity();
        // 已有记录只跳过早回调扩展，不授予任何结果；随后正常分派仍核对完整原身份/方向。
        let mut connection = native
            .source
            .control_pool()
            .acquire()
            .await
            .map_err(|e| DbError::acquisition("XA callback presence", e))?;
        connection.close_on_drop();
        let present: i64 = sqlx::query_scalar("SELECT CAST(EXISTS(SELECT 1 FROM seata_rs_xa_branch_v1 WHERE xid=? AND branch_id=?) AS SIGNED)")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).fetch_one(&mut *connection).await.map_err(|e| DbError::driver("XA callback presence", e))?;
        drop(connection);
        if present == 1 {
            return Ok(());
        }
        let (mut gate, row) = MySqlXaBranchStore::lock(&native.source, id).await?;
        if row.is_some() {
            return gate
                .rollback()
                .await
                .map_err(|e| DbError::driver("XA callback existing release", e));
        }
        let inserted = sqlx::query("INSERT INTO seata_rs_xa_branch_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,?,?,1,2,0)")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes()).bind(id.application_data().map(str::as_bytes)).bind(native.source.server_uuid.as_bytes())
            .execute(&mut *gate).await.map_err(|e| DbError::driver("XA early rollback reservation", e));
        match inserted {
            Ok(result) if result.rows_affected() == 1 => {}
            // 不能从重复键推断回滚；释放本次锁，让正常路径重新观察并发登记的真实状态。
            Err(DbError::Driver {
                code: Some(1062), ..
            }) => {
                return gate
                    .rollback()
                    .await
                    .map_err(|e| DbError::driver("XA early registration race release", e))
            }
            Err(error) => return Err(error),
            _ => return Err(DbError::RecoveryConflict),
        }
        for table in [
            MySqlXaCommitWitness::TABLE,
            MySqlXaPrepareEvidence::TABLE,
            MySqlXaSessionFence::ENROLLMENT,
            MySqlXaSessionFence::GUARD,
        ] {
            if MySqlXaBranchStore::read_table(&mut gate, &native.source, id, table)
                .await?
                .is_some()
            {
                return Err(DbError::RecoveryConflict);
            }
        }
        let xid = crate::MySqlXaResourceManager::xa_id(id);
        if !matches!(native.prepared_observed(&xid).await, Ok(false)) {
            return gate
                .rollback()
                .await
                .map_err(|e| DbError::driver("XA early unresolved release", e));
        }
        let guarded = sqlx::query("INSERT INTO seata_rs_xa_session_guard_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,?,?,1,2,1)")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes()).bind(id.application_data().map(str::as_bytes)).bind(native.source.server_uuid.as_bytes())
            .execute(&mut *gate).await.map_err(|e| DbError::driver("XA early rollback guard", e))?;
        if guarded.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        MySqlXaBranchStore::complete(&mut gate, id, seata_rm::BranchPhase::Rollback).await?;
        gate.commit()
            .await
            .map_err(|e| DbError::driver("XA early rollback control commit", e))
    }
}
