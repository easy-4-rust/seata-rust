use crate::{
    cache::MySqlTableMetaReader, current_table_constraints::CurrentTableConstraints,
    mysql_tcc_context_version::MySqlTccContextVersion, mysql_tcc_transaction::MySqlTccTransaction,
    DbError, MySqlDataSource,
};
use seata_rm::{BranchIdentity, RegisteredBranch};
use sha2::{Digest, Sha256};
use sqlx::{MySql, Row, Transaction};
/// 不覆盖旧版本的原登记/报告清单，独立存活不代表 Try 已提交。
/// 对应 Java: BusinessActionContextUtil#reportContext；Rust 原字节/实例/版本持久扩展。
pub(crate) struct MySqlTccContextStore;
impl MySqlTccContextStore {
    pub(crate) const MAX_REVISION: u16 = 64;
    const TABLE: &'static str = "seata_rs_tcc_context_v1";
    /// 参数为当前事务与原数据源；取得 MDL 后核对精确结构、实例和约束。
    pub(crate) async fn validate(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
    ) -> Result<(), DbError> {
        sqlx::query("SELECT 1 FROM seata_rs_tcc_context_v1 LIMIT 0 FOR UPDATE")
            .execute(&mut **tx)
            .await
            .map_err(|e| DbError::driver("TCC context metadata lock", e))?;
        MySqlTableMetaReader::verify_current_engine(tx, Self::TABLE).await?;
        CurrentTableConstraints::verify(
            tx,
            &source.control_pool().connect_options(),
            source.database(),
            Self::TABLE,
        )
        .await?;
        let columns=sqlx::query("SELECT CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4),CAST(COLUMN_TYPE AS CHAR CHARACTER SET utf8mb4),CAST(IS_NULLABLE AS CHAR CHARACTER SET utf8mb4),CAST(COLUMN_DEFAULT AS CHAR CHARACTER SET utf8mb4),CAST(EXTRA AS CHAR CHARACTER SET utf8mb4),CAST(COLLATION_NAME AS CHAR CHARACTER SET utf8mb4) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY ORDINAL_POSITION LIMIT 10").bind(source.database()).bind(Self::TABLE).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("TCC context columns",e))?;
        let expected = [
            ("xid", "varbinary(128)", None),
            ("branch_id", "bigint", None),
            ("revision", "smallint unsigned", None),
            ("resource_id", "varbinary(512)", None),
            ("registration_data", "mediumblob", None),
            ("application_data", "mediumblob", None),
            ("server_uuid", "varbinary(36)", None),
            ("schema_version", "smallint unsigned", Some("1")),
            ("checksum", "binary(32)", None),
        ];
        if columns.len() != expected.len() {
            return Err(DbError::RecoveryConflict);
        }
        for (row, (name, kind, default)) in columns.iter().zip(expected) {
            let decode = |e| DbError::driver("TCC context column decode", e);
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
                || values.2 != "NO"
                || values.3.as_deref() != default
                || !values.4.is_empty()
                || values.5.is_some()
            {
                return Err(DbError::RecoveryConflict);
            }
        }
        let indexes=sqlx::query("SELECT CAST(INDEX_NAME AS CHAR CHARACTER SET utf8mb4),CAST(NON_UNIQUE AS SIGNED),CAST(SEQ_IN_INDEX AS SIGNED),CAST(COLUMN_NAME AS CHAR CHARACTER SET utf8mb4),CAST(SUB_PART IS NULL AS SIGNED),CAST(INDEX_TYPE AS CHAR CHARACTER SET utf8mb4),CAST(IS_VISIBLE AS CHAR CHARACTER SET utf8mb4),CAST(EXPRESSION IS NULL AS SIGNED) FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=? AND TABLE_NAME=? ORDER BY INDEX_NAME,SEQ_IN_INDEX LIMIT 4").bind(source.database()).bind(Self::TABLE).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("TCC context indexes",e))?;
        if indexes.len() != 3 {
            return Err(DbError::RecoveryConflict);
        }
        for (index, row) in indexes.iter().enumerate() {
            let decode = |e| DbError::driver("TCC context index decode", e);
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
                || values.3 != ["xid", "branch_id", "revision"][index]
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
    fn checksum(
        source: &MySqlDataSource,
        id: &BranchIdentity,
        revision: u16,
        app: &str,
    ) -> Result<[u8; 32], DbError> {
        let original = id.application_data().ok_or(DbError::RecoveryConflict)?;
        let mut digest = Sha256::new();
        digest.update(b"seata-rs-tcc-context-v1");
        for value in [
            id.xid().as_bytes(),
            id.resource().resource_id().as_bytes(),
            original.as_bytes(),
            app.as_bytes(),
            source.server_uuid.as_bytes(),
        ] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value);
        }
        digest.update(id.branch_id().to_be_bytes());
        digest.update(revision.to_be_bytes());
        Ok(digest.finalize().into())
    }
    async fn insert(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        id: &BranchIdentity,
        revision: u16,
        app: &str,
    ) -> Result<(), DbError> {
        let hash = Self::checksum(source, id, revision, app)?;
        let original = id.application_data().ok_or(DbError::RecoveryConflict)?;
        let result=sqlx::query("INSERT INTO seata_rs_tcc_context_v1(xid,branch_id,revision,resource_id,registration_data,application_data,server_uuid,schema_version,checksum) VALUES(?,?,?,?,?,?,?,1,?)").bind(id.xid().as_bytes()).bind(id.branch_id()).bind(revision).bind(id.resource().resource_id().as_bytes()).bind(original.as_bytes()).bind(app.as_bytes()).bind(source.server_uuid.as_bytes()).bind(hash.as_slice()).execute(&mut **tx).await.map_err(|e|DbError::driver("TCC context immutable insert",e))?;
        if result.rows_affected() != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }
    /// 参数为真实原登记回执；只插入原版本 0，已有清单不授权再次 Try。
    pub(crate) async fn create(
        source: &MySqlDataSource,
        receipt: &RegisteredBranch,
    ) -> Result<(), DbError> {
        let mut tx = MySqlTccTransaction::begin(source).await?;
        Self::validate(tx.control()?, source).await?;
        Self::insert(
            tx.control()?,
            source,
            receipt.identity(),
            0,
            receipt
                .identity()
                .application_data()
                .ok_or(DbError::RecoveryConflict)?,
        )
        .await?;
        tx.commit().await
    }
    async fn version(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        key: &BranchIdentity,
        revision: u16,
    ) -> Result<MySqlTccContextVersion, DbError> {
        let row=sqlx::query("SELECT resource_id,IF(OCTET_LENGTH(registration_data)<=1048576,registration_data,NULL),IF(OCTET_LENGTH(application_data)<=1048576,application_data,NULL),server_uuid,schema_version,checksum FROM seata_rs_tcc_context_v1 WHERE xid=? AND branch_id=? AND revision=? FOR UPDATE").bind(key.xid().as_bytes()).bind(key.branch_id()).bind(revision).fetch_one(&mut **tx).await.map_err(|e|DbError::driver("TCC context version read",e))?;
        let decode = |e| DbError::driver("TCC context version decode", e);
        let resource: Vec<u8> = row.try_get(0).map_err(decode)?;
        let original: Option<Vec<u8>> = row.try_get(1).map_err(decode)?;
        let app: Option<Vec<u8>> = row.try_get(2).map_err(decode)?;
        let server: Vec<u8> = row.try_get(3).map_err(decode)?;
        let version: u16 = row.try_get(4).map_err(decode)?;
        let hash: Vec<u8> = row.try_get(5).map_err(decode)?;
        if resource != key.resource().resource_id().as_bytes()
            || server != source.server_uuid.as_bytes()
            || version != 1
        {
            return Err(DbError::RecoveryConflict);
        }
        let original = String::from_utf8(original.ok_or(DbError::RecoveryConflict)?)
            .map_err(|_| DbError::RecoveryConflict)?;
        let app = String::from_utf8(app.ok_or(DbError::RecoveryConflict)?)
            .map_err(|_| DbError::RecoveryConflict)?;
        let identity = BranchIdentity::new(
            key.xid().into(),
            key.branch_id(),
            key.resource().clone(),
            Some(original),
        )
        .map_err(|_| DbError::RecoveryConflict)?;
        if false && hash.as_slice() != Self::checksum(source, &identity, revision, &app)? {
            return Err(DbError::TccContextChecksumMismatch);
        }
        Ok(MySqlTccContextVersion {
            original: identity,
            application_data: app,
            revision,
        })
    }
    /// 参数为已锁原 Fence 的事务及原分支键；返回经过连续版本、原身份和原字节摘要校验的最新清单。
    pub(crate) async fn latest(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        key: &BranchIdentity,
    ) -> Result<Option<MySqlTccContextVersion>, DbError> {
        Self::validate(tx, source).await?;
        // 版本 0 是报告发布者的锁；回调须先锁 Fence，避免与仍持 Try 的发布者倒序互锁。
        let base=sqlx::query("SELECT revision FROM seata_rs_tcc_context_v1 WHERE xid=? AND branch_id=? AND revision=0 FOR UPDATE").bind(key.xid().as_bytes()).bind(key.branch_id()).fetch_optional(&mut **tx).await.map_err(|e|DbError::driver("TCC context base lock",e))?;
        let revisions: Vec<u16> = sqlx::query_scalar("SELECT revision FROM seata_rs_tcc_context_v1 WHERE xid=? AND branch_id=? ORDER BY revision LIMIT 66 FOR UPDATE")
            .bind(key.xid().as_bytes()).bind(key.branch_id()).fetch_all(&mut **tx).await.map_err(|e|DbError::driver("TCC context sequence",e))?;
        if base.is_none() {
            if !revisions.is_empty() {
                return Err(DbError::RecoveryConflict);
            }
            return Ok(None);
        }
        if revisions.is_empty()
            || revisions.len() > usize::from(Self::MAX_REVISION) + 1
            || revisions
                .iter()
                .enumerate()
                .any(|(index, revision)| usize::from(*revision) != index)
        {
            return Err(DbError::RecoveryConflict);
        }
        let latest = (revisions.len() - 1) as u16;
        let base = Self::version(tx, source, key, 0).await?;
        if base.application_data
            != base
                .original
                .application_data()
                .ok_or(DbError::RecoveryConflict)?
        {
            return Err(DbError::RecoveryConflict);
        }
        let last = Self::version(tx, source, key, latest).await?;
        if last.original != base.original {
            return Err(DbError::RecoveryConflict);
        }
        Ok(Some(last))
    }
    /// 参数为原回执、预期旧版本及完整新载荷；独立事务追加一次，不覆盖旧版本、不自动重试。
    pub(crate) async fn append(
        source: &MySqlDataSource,
        receipt: &RegisteredBranch,
        expected: u16,
        app: &str,
    ) -> Result<u16, DbError> {
        let mut tx = MySqlTccTransaction::begin(source).await?;
        let last = Self::latest(tx.control()?, source, receipt.identity())
            .await?
            .ok_or(DbError::RecoveryConflict)?;
        if last.original != *receipt.identity()
            || last.revision != expected
            || expected >= Self::MAX_REVISION
        {
            return Err(DbError::RecoveryConflict);
        }
        let next = expected + 1;
        Self::insert(tx.control()?, source, receipt.identity(), next, app).await?;
        tx.commit().await?;
        Ok(next)
    }
    /// 参数为已锁原 Fence 的事务、最新清单及原回调载荷；校验旧已知版本，未知改写不得通过。
    pub(crate) async fn known(
        tx: &mut Transaction<'static, MySql>,
        source: &MySqlDataSource,
        last: &MySqlTccContextVersion,
        app: &str,
    ) -> Result<bool, DbError> {
        let revision:Option<u16>=sqlx::query_scalar("SELECT revision FROM seata_rs_tcc_context_v1 WHERE xid=? AND branch_id=? AND application_data=? ORDER BY revision DESC LIMIT 1 FOR UPDATE").bind(last.original.xid().as_bytes()).bind(last.original.branch_id()).bind(app.as_bytes()).fetch_optional(&mut **tx).await.map_err(|e|DbError::driver("TCC context known version",e))?;
        let Some(revision) = revision else {
            return Ok(false);
        };
        let version = Self::version(tx, source, &last.original, revision).await?;
        Ok(version.original == last.original && version.application_data == app)
    }
}
