use crate::{mysql_xa_branch_store::MySqlXaBranchStore, DbError, MySqlDataSource};
use seata_rm::BranchIdentity;
use sqlx::MySqlConnection;

/// 原 XA 与业务共同提交的正向见证；缺行不能证明原事务未提交或已回滚。
/// 对应 Java: 无直接对象；Rust XA 响应丢失后的同事务提交证明扩展。
pub(crate) struct MySqlXaCommitWitness;
impl MySqlXaCommitWitness {
    pub(crate) const TABLE: &'static str = "seata_rs_xa_commit_witness_v1";

    /// 参数为原 ACTIVE 物理连接、数据源和原登记；在原 XA 内唯一插入精确身份，不另行提交。
    pub(crate) async fn insert(
        connection: &mut MySqlConnection,
        source: &MySqlDataSource,
        identity: &BranchIdentity,
    ) -> Result<(), DbError> {
        let result = sqlx::query("INSERT INTO seata_rs_xa_commit_witness_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,?,?,1,1,1)")
            .bind(identity.xid().as_bytes()).bind(identity.branch_id())
            .bind(identity.resource().resource_id().as_bytes())
            .bind(identity.application_data().map(str::as_bytes)).bind(source.server_uuid.as_bytes())
            .execute(connection).await.map_err(|e| DbError::driver("XA commit witness insert", e))?;
        if result.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }

    /// 参数为原数据源和原登记；只接纳当前物理表中逐字段匹配的已提交见证，返回正向证明。
    pub(crate) async fn committed(
        source: &MySqlDataSource,
        identity: &BranchIdentity,
    ) -> Result<bool, DbError> {
        let (gate, record) = MySqlXaBranchStore::lock_table(source, identity, Self::TABLE).await?;
        let found = match record {
            Some(value) if value.phase == 1 && value.completed => true,
            Some(_) => return Err(DbError::RecoveryConflict),
            None => false,
        };
        gate.rollback()
            .await
            .map_err(|e| DbError::driver("XA witness read release", e))?;
        Ok(found)
    }
}
