use crate::{mysql_xa_branch_store::MySqlXaBranchStore, DbError, MySqlDataSource};
use seata_rm::BranchIdentity;
use sqlx::{MySql, MySqlConnection, Transaction};

/// 原协议登记和同一唯一键上的事务内 guard/持久回滚屏障，不按会话编号终止其他连接。
/// 对应 Java: 无直接对象；Rust XA 旧活动作用域的资源级围栏扩展。
pub(crate) struct MySqlXaSessionFence;
impl MySqlXaSessionFence {
    pub(crate) const ENROLLMENT: &'static str = "seata_rs_xa_guard_enrollment_v1";
    pub(crate) const GUARD: &'static str = "seata_rs_xa_session_guard_v1";

    /// 参数为原登记控制事务、数据源和身份；与唯一登记原子签发新 guard 协议，禁止回填旧分支。
    pub(crate) async fn enroll(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        id: &BranchIdentity,
    ) -> Result<(), DbError> {
        MySqlXaBranchStore::validate_table(tx, source, Self::ENROLLMENT).await?;
        MySqlXaBranchStore::validate_table(tx, source, Self::GUARD).await?;
        let result = sqlx::query("INSERT INTO seata_rs_xa_guard_enrollment_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,?,?,1,0,0)")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes()).bind(id.application_data().map(str::as_bytes)).bind(source.server_uuid.as_bytes())
            .execute(&mut **tx).await.map_err(|e| DbError::driver("XA guard enrollment", e))?;
        if result.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }

    /// 参数为原 ACTIVE 连接和登记；原作用域返回及任何业务之前，唯一写入未提交 guard。
    pub(crate) async fn insert_guard(
        connection: &mut MySqlConnection,
        source: &MySqlDataSource,
        id: &BranchIdentity,
    ) -> Result<(), DbError> {
        let result = sqlx::query("INSERT INTO seata_rs_xa_session_guard_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,?,?,1,0,0)")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes()).bind(id.application_data().map(str::as_bytes)).bind(source.server_uuid.as_bytes())
            .execute(connection).await.map_err(|e| DbError::driver("XA active guard insert", e))?;
        if result.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }

    /// 参数为持原方向门闩的控制事务和原身份；等待旧 guard 释放并持久竞争屏障，调用者必须重查状态后提交。
    pub(crate) async fn fence(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        id: &BranchIdentity,
    ) -> Result<bool, DbError> {
        let enrollment = MySqlXaBranchStore::read_table(tx, source, id, Self::ENROLLMENT).await?;
        match enrollment {
            None => return Ok(false),
            Some(row) if row.phase != 0 || row.completed => return Err(DbError::RecoveryConflict),
            Some(_) => {}
        }
        MySqlXaBranchStore::validate_table(tx, source, Self::GUARD).await?;
        let result = sqlx::query("INSERT INTO seata_rs_xa_session_guard_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,?,?,1,2,1)")
            .bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes()).bind(id.application_data().map(str::as_bytes)).bind(source.server_uuid.as_bytes())
            .execute(&mut **tx).await.map_err(|e| DbError::driver("XA rollback guard fence", e));
        match result {
            Ok(result) if result.rows_affected() == 1 => {}
            Err(DbError::Driver {
                code: Some(1062), ..
            }) => {}
            Err(error) => return Err(error),
            _ => return Err(DbError::RecoveryConflict),
        }
        let guard = MySqlXaBranchStore::read_table(tx, source, id, Self::GUARD).await?;
        if !guard.is_some_and(|row| row.phase == 2 && row.completed) {
            return Err(DbError::RecoveryConflict);
        }
        Ok(true)
    }
}
