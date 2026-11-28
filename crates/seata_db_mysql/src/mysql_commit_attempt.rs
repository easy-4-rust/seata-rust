use crate::DbError;
use seata_at::MySqlJdbcProfile;
use seata_protocol::{BranchType, ResultCode, TransactionExceptionCode};
use seata_rm::BranchIdentity;
use seata_transport::ResourceKey;
use sqlx::{mysql::MySqlRow, Row};

/// 从 006 持久记录核验后读取的原提交尝试；不签发新的 TC 登记回执。
/// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 的原分支身份与本地提交意图。
#[derive(Debug)]
pub struct MySqlCommitAttempt {
    identity: BranchIdentity,
    lock_key: Option<String>,
    profile: MySqlJdbcProfile,
    response_exception_code: TransactionExceptionCode,
}

impl MySqlCommitAttempt {
    /// 参数为受 006 表结构保护的当前行、原查询身份、资源及配置；返回逐字节核验的尝试。
    pub(crate) fn from_row(
        row: &MySqlRow,
        xid: &str,
        branch_id: i64,
        resource: &ResourceKey,
        profile: MySqlJdbcProfile,
    ) -> Result<Self, DbError> {
        let bytes = |name| {
            row.try_get::<Vec<u8>, _>(name)
                .map_err(|error| DbError::driver("commit attempt read bytes", error))
        };
        let optional = |name| {
            row.try_get::<Option<Vec<u8>>, _>(name)
                .map_err(|error| DbError::driver("commit attempt read optional", error))
        };
        let number = |name| {
            row.try_get::<u8, _>(name)
                .map_err(|error| DbError::driver("commit attempt read number", error))
        };
        let stored_branch: i64 = row
            .try_get("branch_id")
            .map_err(|error| DbError::driver("commit attempt read branch", error))?;
        let stored_version: u16 = row
            .try_get("schema_version")
            .map_err(|error| DbError::driver("commit attempt read version", error))?;
        let stored_profile = MySqlJdbcProfile::from_storage_code(number("jdbc_profile")?)
            .ok_or(DbError::RecoveryConflict)?;
        if stored_version != 1
            || stored_branch != branch_id
            || bytes("xid")? != xid.as_bytes()
            || bytes("resource_id")? != resource.resource_id().as_bytes()
            || stored_profile != profile
            || number("response_result_code")? != ResultCode::Success.to_java_v1()
            || number("attempt_state")? != 1
            || number("report_started")? > 1
        {
            return Err(DbError::RecoveryConflict);
        }
        let decode =
            |bytes: Vec<u8>| String::from_utf8(bytes).map_err(|_| DbError::RecoveryConflict);
        let application_data = optional("application_data")?.map(decode).transpose()?;
        let lock_key = optional("lock_key")?.map(decode).transpose()?;
        let identity = BranchIdentity::new(
            xid.to_owned(),
            branch_id,
            ResourceKey::new(BranchType::At, resource.resource_id().to_owned())
                .map_err(|_| DbError::RecoveryConflict)?,
            application_data,
        )
        .map_err(|_| DbError::RecoveryConflict)?;
        let response_exception_code =
            TransactionExceptionCode::from_java_v1(number("response_exception_code")?)
                .map_err(|_| DbError::RecoveryConflict)?;
        Ok(Self {
            identity,
            lock_key,
            profile,
            response_exception_code,
        })
    }

    /// 无参数；返回原 XID、BranchId、资源及应用数据的只读身份。
    pub fn identity(&self) -> &BranchIdentity {
        &self.identity
    }

    /// 无参数；返回原登记申请的可选锁键，不据此重新申请 BranchId。
    pub fn lock_key(&self) -> Option<&str> {
        self.lock_key.as_deref()
    }

    /// 无参数；返回写入时固定的 JDBC 配置。
    pub const fn profile(&self) -> MySqlJdbcProfile {
        self.profile
    }

    /// 无参数；返回原成功响应中的异常码，供恢复审计核对。
    pub const fn response_exception_code(&self) -> TransactionExceptionCode {
        self.response_exception_code
    }
}
