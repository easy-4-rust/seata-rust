use crate::{
    cache::MySqlTableMetaReader, current_table_constraints::CurrentTableConstraints,
    mysql_xa_branch_record::MySqlXaBranchRecord, DbError, MySqlDataSource,
};
use seata_rm::{BranchIdentity, BranchPhase, RegisteredBranch};
use sqlx::{MySql, Row, Transaction};
/// 原成功登记的单次启动门闩，以及跨 RM 进程不换向的原生 XA 控制存储。
/// 对应 Java: 无直接对象；Rust XA 的持久原身份和方向仲裁扩展。
pub(crate) struct MySqlXaBranchStore;
impl MySqlXaBranchStore {
    pub(crate) const TABLE: &'static str = "seata_rs_xa_branch_v1";
    /// 参数为原数据源；返回退出即关闭的独占控制事务，由外层统一绝对期限约束。
    async fn begin(source: &MySqlDataSource) -> Result<Transaction<'static, MySql>, DbError> {
        let mut connection = source
            .control_pool()
            .acquire()
            .await
            .map_err(|e| DbError::acquisition("XA branch acquire", e))?;
        connection.close_on_drop();
        Transaction::begin(connection, None)
            .await
            .map_err(|e| DbError::driver("XA branch begin", e))
    }
    /// 参数为原控制事务和数据源；核对 MDL 下的精确当前物理形状，不自动迁移。
    async fn validate(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
    ) -> Result<(), DbError> {
        Self::validate_table(tx, source, Self::TABLE).await
    }
    /// 参数为原控制事务、数据源和内部固定表名；校验相同的不可变身份物理存储契约。
    pub(crate) async fn validate_table(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        table: &'static str,
    ) -> Result<(), DbError> {
        if table != Self::TABLE
            && table != crate::mysql_xa_commit_witness::MySqlXaCommitWitness::TABLE
        {
            return Err(DbError::RecoveryConflict);
        }
        sqlx::query(&format!("SELECT 1 FROM {table} LIMIT 0 FOR UPDATE"))
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("XA branch metadata lock", e))?;
        MySqlTableMetaReader::verify_current_engine(tx, table).await?;
        CurrentTableConstraints::verify(
            tx,
            &source.control_pool().connect_options(),
            source.database(),
            table,
        )
        .await?;
        let durable: i64 =
            sqlx::query_scalar("SELECT CAST(@@GLOBAL.innodb_flush_log_at_trx_commit AS SIGNED)")
                .fetch_one(&mut **tx)
                .await
                .map_err(|e| DbError::driver("XA branch durability", e))?;
        if durable != 1 {
            return Err(DbError::Unsupported {
                feature: "XA branch requires durable InnoDB commit",
            });
        }
        let columns=sqlx::query("SELECT CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4),CAST(COLUMN_TYPE AS CHAR CHARACTER SET utf8mb4),CAST(IS_NULLABLE AS CHAR CHARACTER SET utf8mb4),CAST(COLUMN_DEFAULT AS CHAR CHARACTER SET utf8mb4),CAST(EXTRA AS CHAR CHARACTER SET utf8mb4),CAST(COLLATION_NAME AS CHAR CHARACTER SET utf8mb4) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY ORDINAL_POSITION LIMIT 9").bind(source.database()).bind(table).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("XA branch columns",e))?;
        let expected = [
            ("xid", "varbinary(64)", false, None),
            ("branch_id", "bigint", false, None),
            ("resource_id", "varbinary(512)", false, None),
            ("application_data", "mediumblob", true, None),
            ("server_uuid", "varbinary(36)", false, None),
            ("schema_version", "smallint unsigned", false, Some("1")),
            ("phase", "tinyint unsigned", false, Some("0")),
            ("completed", "tinyint unsigned", false, Some("0")),
        ];
        if columns.len() != expected.len() {
            return Err(DbError::RecoveryConflict);
        }
        for (row, (name, kind, nullable, default)) in columns.iter().zip(expected) {
            let decode = |e| DbError::driver("XA column decode", e);
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
        let indexes=sqlx::query("SELECT CAST(INDEX_NAME AS CHAR CHARACTER SET utf8mb4) AS INDEX_NAME,CAST(NON_UNIQUE AS SIGNED) AS n,CAST(SEQ_IN_INDEX AS SIGNED) AS s,CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4) AS COLUMN_NAME,CAST(SUB_PART IS NULL AS SIGNED) AS full_column,CAST(INDEX_TYPE AS CHAR CHARACTER SET utf8mb4) AS INDEX_TYPE,CAST(IS_VISIBLE AS CHAR CHARACTER SET utf8mb4) AS IS_VISIBLE,CAST(EXPRESSION IS NULL AS SIGNED) AS plain FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY INDEX_NAME,SEQ_IN_INDEX LIMIT 3").bind(source.database()).bind(table).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("XA branch index",e))?;
        if indexes.len() != 2 {
            return Err(DbError::RecoveryConflict);
        }
        for (index, row) in indexes.iter().enumerate() {
            let name: String = row
                .try_get("INDEX_NAME")
                .map_err(|e| DbError::driver("XA index decode", e))?;
            let column: String = row
                .try_get("COLUMN_NAME")
                .map_err(|e| DbError::driver("XA index decode", e))?;
            let n: i64 = row
                .try_get("n")
                .map_err(|e| DbError::driver("XA index decode", e))?;
            let sequence: i64 = row
                .try_get("s")
                .map_err(|e| DbError::driver("XA index decode", e))?;
            let full: i64 = row
                .try_get("full_column")
                .map_err(|e| DbError::driver("XA index decode", e))?;
            let plain: i64 = row
                .try_get("plain")
                .map_err(|e| DbError::driver("XA index decode", e))?;
            let kind: String = row
                .try_get("INDEX_TYPE")
                .map_err(|e| DbError::driver("XA index decode", e))?;
            let visible: String = row
                .try_get("IS_VISIBLE")
                .map_err(|e| DbError::driver("XA index decode", e))?;
            if name != "PRIMARY"
                || column != ["xid", "branch_id"][index]
                || n != 0
                || sequence != index as i64 + 1
                || full != 1
                || plain != 1
                || kind != "BTREE"
                || visible != "YES"
            {
                return Err(DbError::RecoveryConflict);
            }
        }
        Ok(())
    }
    /// 参数为原数据源及精确身份；返回持原行锁的控制事务和可选已验证快照。
    pub(crate) async fn lock(
        source: &MySqlDataSource,
        id: &BranchIdentity,
    ) -> Result<(Transaction<'static, MySql>, Option<MySqlXaBranchRecord>), DbError> {
        Self::lock_table(source, id, Self::TABLE).await
    }
    /// 参数为原身份及固定内部表；返回持锁且逐字段验证的原登记或原提交见证。
    pub(crate) async fn lock_table(
        source: &MySqlDataSource,
        id: &BranchIdentity,
        table: &'static str,
    ) -> Result<(Transaction<'static, MySql>, Option<MySqlXaBranchRecord>), DbError> {
        let mut tx = Self::begin(source).await?;
        Self::validate_table(&mut tx, source, table).await?;
        let row=sqlx::query(&format!("SELECT resource_id,server_uuid,schema_version,phase,completed,CAST(application_data <=> ? AS SIGNED) AS app_matches,CAST(application_data IS NULL OR OCTET_LENGTH(application_data)<=1048576 AS SIGNED) AS app_bounded FROM {table} WHERE xid=? AND branch_id=? FOR UPDATE")).bind(id.application_data().map(str::as_bytes)).bind(id.xid().as_bytes()).bind(id.branch_id()).fetch_optional(&mut *tx).await.map_err(|e|DbError::driver("XA branch read",e))?;
        let Some(row) = row else {
            return Ok((tx, None));
        };
        let resource: Vec<u8> = row
            .try_get("resource_id")
            .map_err(|e| DbError::driver("XA branch identity", e))?;
        let server: Vec<u8> = row
            .try_get("server_uuid")
            .map_err(|e| DbError::driver("XA branch identity", e))?;
        let version: u16 = row
            .try_get("schema_version")
            .map_err(|e| DbError::driver("XA branch version", e))?;
        let phase: u8 = row
            .try_get("phase")
            .map_err(|e| DbError::driver("XA branch phase", e))?;
        let completed: u8 = row
            .try_get("completed")
            .map_err(|e| DbError::driver("XA branch terminal", e))?;
        let app: i64 = row
            .try_get("app_matches")
            .map_err(|e| DbError::driver("XA branch application", e))?;
        let bounded: i64 = row
            .try_get("app_bounded")
            .map_err(|e| DbError::driver("XA branch application", e))?;
        if resource != id.resource().resource_id().as_bytes()
            || server != source.server_uuid.as_bytes()
            || version != 1
            || phase > 2
            || completed > 1
            || (completed == 1 && phase == 0)
            || app != 1
            || bounded != 1
        {
            return Err(DbError::RecoveryConflict);
        }
        Ok((
            tx,
            Some(MySqlXaBranchRecord {
                phase,
                completed: completed == 1,
            }),
        ))
    }
    /// 参数为不可伪造的原登记回执；确认唯一插入 COMMIT 后才允许该原身份开始一次 XA。
    pub(crate) async fn claim(
        source: &MySqlDataSource,
        receipt: &RegisteredBranch,
    ) -> Result<(), DbError> {
        let mut tx = Self::begin(source).await?;
        Self::validate(&mut tx, source).await?;
        let id = receipt.identity();
        let changed=sqlx::query("INSERT INTO seata_rs_xa_branch_v1(xid,branch_id,resource_id,application_data,server_uuid,schema_version,phase,completed) VALUES(?,?,?,?,?,1,0,0)").bind(id.xid().as_bytes()).bind(id.branch_id()).bind(id.resource().resource_id().as_bytes()).bind(id.application_data().map(str::as_bytes)).bind(source.server_uuid.as_bytes()).execute(&mut *tx).await.map_err(|e|DbError::driver("XA branch claim",e))?;
        if changed.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        tx.commit()
            .await
            .map_err(|e| DbError::driver("XA branch claim commit", e))
    }
    /// 参数为原身份和原决定；在任何原生结束命令前持久绑定单一方向，缺行保持未知。
    pub(crate) async fn bind(
        source: &MySqlDataSource,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<bool, DbError> {
        let (mut tx, row) = Self::lock(source, id).await?;
        let Some(row) = row else {
            tx.rollback()
                .await
                .map_err(|e| DbError::driver("XA branch missing rollback", e))?;
            return Ok(false);
        };
        let wanted = Self::phase(phase);
        if row.phase != 0 && row.phase != wanted {
            return Err(DbError::RecoveryConflict);
        }
        if row.phase == 0 {
            let changed=sqlx::query("UPDATE seata_rs_xa_branch_v1 SET phase=? WHERE xid=? AND branch_id=? AND phase=0 AND completed=0").bind(wanted).bind(id.xid().as_bytes()).bind(id.branch_id()).execute(&mut *tx).await.map_err(|e|DbError::driver("XA branch bind",e))?;
            if changed.rows_affected() != 1 {
                return Err(DbError::RecoveryConflict);
            }
        }
        tx.commit()
            .await
            .map_err(|e| DbError::driver("XA branch bind commit", e))?;
        Ok(true)
    }
    /// 参数为持原行锁的控制事务、原身份及同方向；仅原生成功响应之后由调用者确认终态。
    pub(crate) async fn complete(
        tx: &mut Transaction<'static, MySql>,
        id: &BranchIdentity,
        phase: BranchPhase,
    ) -> Result<(), DbError> {
        let changed=sqlx::query("UPDATE seata_rs_xa_branch_v1 SET completed=1 WHERE xid=? AND branch_id=? AND phase=? AND completed=0").bind(id.xid().as_bytes()).bind(id.branch_id()).bind(Self::phase(phase)).execute(&mut **tx).await.map_err(|e|DbError::driver("XA branch complete",e))?;
        if changed.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }
    /// 参数为原二阶段决定；返回持久非零方向编号。
    pub(crate) const fn phase(phase: BranchPhase) -> u8 {
        match phase {
            BranchPhase::Commit => 1,
            BranchPhase::Rollback => 2,
        }
    }
}
