use crate::{
    cache::MySqlTableMetaReader, current_table_constraints::CurrentTableConstraints, DbError,
    MySqlDataSource,
};
use seata_rm::BranchIdentity;
use seata_tcc::CommonFenceStatus;
use sqlx::{MySql, Row, Transaction};
/// 持原行锁的 TCC Fence 与不可变原登记身份，不自动迁移或清理终态。
/// 对应 Java: org.apache.seata.integration.tx.api.fence.store.db.CommonFenceStoreDataBaseDAO；Rust 原身份/实例校验扩展。
pub(crate) struct MySqlTccFenceStore;
impl MySqlTccFenceStore {
    pub(crate) const TABLE: &'static str = "seata_rs_tcc_fence_v1";
    /// 参数为原事务和数据源；持 MDL 检查精确结构、当前实例、约束与提交持久性。
    pub(crate) async fn validate(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
    ) -> Result<(), DbError> {
        sqlx::query("SELECT 1 FROM seata_rs_tcc_fence_v1 LIMIT 0 FOR UPDATE")
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("TCC Fence metadata lock", e))?;
        MySqlTableMetaReader::verify_current_engine(tx, Self::TABLE).await?;
        CurrentTableConstraints::verify(
            tx,
            &source.control_pool().connect_options(),
            source.database(),
            Self::TABLE,
        )
        .await?;
        let current:(String,String,i64)=sqlx::query_as("SELECT @@server_uuid,DATABASE(),CAST(@@GLOBAL.innodb_flush_log_at_trx_commit AS SIGNED)").fetch_one(&mut **tx).await.map_err(|e|DbError::driver("TCC Fence current identity",e))?;
        if current.0 != source.server_uuid.as_ref()
            || current.1 != source.database()
            || current.2 != 1
        {
            return Err(DbError::RecoveryConflict);
        }
        let columns=sqlx::query("SELECT CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4),CAST(COLUMN_TYPE AS CHAR CHARACTER SET utf8mb4),CAST(IS_NULLABLE AS CHAR CHARACTER SET utf8mb4),CAST(COLUMN_DEFAULT AS CHAR CHARACTER SET utf8mb4),CAST(EXTRA AS CHAR CHARACTER SET utf8mb4),CAST(COLLATION_NAME AS CHAR CHARACTER SET utf8mb4) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY ORDINAL_POSITION LIMIT 8").bind(source.database()).bind(Self::TABLE).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("TCC Fence columns",e))?;
        let expected = [
            ("xid", "varbinary(128)", false, None),
            ("branch_id", "bigint", false, None),
            ("resource_id", "varbinary(512)", false, None),
            ("application_data", "mediumblob", true, None),
            ("server_uuid", "varbinary(36)", false, None),
            ("schema_version", "smallint unsigned", false, Some("1")),
            ("status", "tinyint unsigned", false, None),
        ];
        if columns.len() != expected.len() {
            return Err(DbError::RecoveryConflict);
        }
        for (row, (name, kind, nullable, default)) in columns.iter().zip(expected) {
            let decode = |e| DbError::driver("TCC Fence column decode", e);
            let values: (
                String,
                String,
                String,
                Option<String>,
                String,
                Option<String>,
            ) = (
                row.try_get(0).map_err(decode)?,
                row.try_get(1).map_err(decode)?,
                row.try_get(2).map_err(decode)?,
                row.try_get(3).map_err(decode)?,
                row.try_get(4).map_err(decode)?,
                row.try_get(5).map_err(decode)?,
            );
            if values.0 != name
                || values.1 != kind
                || values.2 != if nullable { "YES" } else { "NO" }
                || values.3.as_deref() != default
                || !values.4.is_empty()
                || values.5.is_some()
            {
                return Err(DbError::RecoveryConflict);
            }
        }
        let indexes=sqlx::query("SELECT CAST(INDEX_NAME AS CHAR CHARACTER SET utf8mb4),CAST(NON_UNIQUE AS SIGNED),CAST(SEQ_IN_INDEX AS SIGNED),CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4),CAST(SUB_PART IS NULL AS SIGNED),CAST(INDEX_TYPE AS CHAR CHARACTER SET utf8mb4),CAST(IS_VISIBLE AS CHAR CHARACTER SET utf8mb4),CAST(EXPRESSION IS NULL AS SIGNED) FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY INDEX_NAME,SEQ_IN_INDEX LIMIT 3").bind(source.database()).bind(Self::TABLE).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("TCC Fence indexes",e))?;
        if indexes.len() != 2 {
            return Err(DbError::RecoveryConflict);
        }
        for (index, row) in indexes.iter().enumerate() {
            let decode = |e| DbError::driver("TCC Fence index decode", e);
            let values: (String, i64, i64, String, i64, String, String, i64) = (
                row.try_get(0).map_err(decode)?,
                row.try_get(1).map_err(decode)?,
                row.try_get(2).map_err(decode)?,
                row.try_get(3).map_err(decode)?,
                row.try_get(4).map_err(decode)?,
                row.try_get(5).map_err(decode)?,
                row.try_get(6).map_err(decode)?,
                row.try_get(7).map_err(decode)?,
            );
            if values.0 != "PRIMARY"
                || values.1 != 0
                || values.2 != index as i64 + 1
                || values.3 != ["xid", "branch_id"][index]
                || values.4 != 1
                || values.5 != "BTREE"
                || values.6 != "YES"
                || values.7 != 1
            {
                return Err(DbError::RecoveryConflict);
            }
        }
        Ok(())
    }
    /// 参数为已验证事务、原身份和预期协议（静态 1/报告 2）；返回持锁 Fence 状态，应用 BLOB 仅在服务器比较，拒绝超限和身份冲突。
    pub(crate) async fn read(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        id: &BranchIdentity,
        protocol_version: u16,
    ) -> Result<Option<CommonFenceStatus>, DbError> {
        if !matches!(protocol_version, 1 | 2) {
            return Err(DbError::RecoveryConflict);
        }
        let row=sqlx::query("SELECT resource_id,server_uuid,schema_version,status,CAST(application_data <=> ? AS SIGNED),CAST(application_data IS NULL OR OCTET_LENGTH(application_data)<=1048576 AS SIGNED) FROM seata_rs_tcc_fence_v1 WHERE xid=? AND branch_id=? FOR UPDATE").bind(id.application_data().map(str::as_bytes)).bind(id.xid().as_bytes()).bind(id.branch_id()).fetch_optional(&mut **tx).await.map_err(|e|DbError::driver("TCC Fence read",e))?;
        let Some(row) = row else { return Ok(None) };
        let decode = |e| DbError::driver("TCC Fence identity decode", e);
        let resource: Vec<u8> = row.try_get(0).map_err(decode)?;
        let server: Vec<u8> = row.try_get(1).map_err(decode)?;
        let version: u16 = row.try_get(2).map_err(decode)?;
        let status: u8 = row.try_get(3).map_err(decode)?;
        let app: i64 = row.try_get(4).map_err(decode)?;
        let bounded: i64 = row.try_get(5).map_err(decode)?;
        if resource != id.resource().resource_id().as_bytes()
            || server != source.server_uuid.as_bytes()
            || !matches!(version,1|2)
            || app != 1
            || bounded != 1
        {
            return Err(DbError::RecoveryConflict);
        }
        CommonFenceStatus::try_from(status as i32)
            .map(Some)
            .map_err(|_| DbError::RecoveryConflict)
    }
    /// 参数为原身份、初始状态及预期协议；唯一插入仲裁 Try/空 Cancel，不忽略重复键或覆盖已有记录。
    pub(crate) async fn insert(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        id: &BranchIdentity,
        status: CommonFenceStatus,
        protocol_version: u16,
    ) -> Result<(), DbError> {
        if !matches!(protocol_version, 1 | 2) {
            return Err(DbError::RecoveryConflict);
        }
        let result=sqlx::query("INSERT INTO seata_rs_tcc_fence_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,status) VALUES(?,?,?,?,?,?,?)").bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes()).bind(id.application_data().map(str::as_bytes)).bind(source.server_uuid.as_bytes()).bind(protocol_version).bind(status.code()).execute(&mut **tx).await.map_err(|e|DbError::driver("TCC Fence unique insert",e))?;
        if result.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }
    /// 参数为原身份及比较/目标状态；成功更新后业务仍须同事务执行和提交。
    pub(crate) async fn update(
        tx: &mut Transaction<'static, MySql>,
        id: &BranchIdentity,
        expected: CommonFenceStatus,
        next: CommonFenceStatus,
    ) -> Result<(), DbError> {
        let result = sqlx::query(
            "UPDATE seata_rs_tcc_fence_v1 SET status=? WHERE xid=? AND branch_id=? AND status=?",
        )
        .bind(next.code())
        .bind(id.xid().as_bytes())
        .bind(id.branch_id())
        .bind(expected.code())
        .execute(&mut **tx)
        .await
        .map_err(|e| DbError::driver("TCC Fence compare update", e))?;
        if result.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }
}
