use crate::{
    DbError, MySqlDataSource, MySqlLauncherClaimError, MySqlLauncherSnapshot,
    MySqlTransferredLauncher,
};
use seata_protocol::{GlobalStatus, TransactionExceptionCode};
use seata_tm::{
    LauncherOrigin, LauncherProcessMonitor, LauncherStopChallenge, LauncherStopVerifier,
    LauncherStore, StoppedLauncherOwner, TmOperation, TransactionManager, Xid,
};
use sha2::{Digest, Sha256};
use sqlx::{Acquire, Row};
use tokio::time::{timeout_at, Instant};

/// 008 表上的 TM Launcher 来源与一次性结束发送门禁；对应 Java: DefaultGlobalTransaction 的持久调用边界。
#[derive(Clone)]
pub struct MySqlLauncherStore {
    data_source: MySqlDataSource,
}
impl MySqlLauncherStore {
    /// 参数为已验证的 MySQL 数据源；返回共享其控制池与操作预算的持久层。
    pub fn new(data_source: MySqlDataSource) -> Self {
        Self { data_source }
    }
    /// 使用可信控制进程保存的原监护器、停止证明和来源快照，申请将代际转给参数管理器。
    /// 仅当原意图已持久化、尚未标记发包且原 TC 返回 Begin 时申请；返回已确认的新代际句柄。
    /// 调用方须使用专用接管账号。超时或确认丢失时返回错误。
    pub async fn transfer_stopped_owner<'a>(
        &self,
        snapshot: &'a MySqlLauncherSnapshot,
        monitor: &LauncherProcessMonitor,
        stopped: &StoppedLauncherOwner,
        manager: &'a TransactionManager,
        business_intent: &str,
    ) -> Result<MySqlTransferredLauncher<'a>, MySqlLauncherClaimError> {
        self.transfer_verified_owner(snapshot, manager, business_intent, || {
            monitor.verifies(stopped, snapshot.owner_instance_id())
        })
        .await
    }

    /// 按部署预配的原主机验证器及当前挑战核验停止签名，再申请代际。
    /// 参数包括已核对的来源、新管理器和原业务意图；成功后返回已提交的新代际句柄。
    pub async fn transfer_attested_owner<'a>(
        &self,
        snapshot: &'a MySqlLauncherSnapshot,
        verifier: &LauncherStopVerifier,
        challenge: &LauncherStopChallenge,
        signature: &[u8; 64],
        manager: &'a TransactionManager,
        business_intent: &str,
    ) -> Result<MySqlTransferredLauncher<'a>, MySqlLauncherClaimError> {
        self.transfer_verified_owner(snapshot, manager, business_intent, || {
            snapshot.owner_stop_scope() == Some(verifier.scope())
                && verifier.verifies(
                    challenge,
                    signature,
                    snapshot.owner_instance_id(),
                    manager.owner_instance_id(),
                )
        })
        .await
    }

    async fn transfer_verified_owner<'a>(
        &self,
        snapshot: &'a MySqlLauncherSnapshot,
        manager: &'a TransactionManager,
        business_intent: &str,
        authorized: impl Fn() -> bool,
    ) -> Result<MySqlTransferredLauncher<'a>, MySqlLauncherClaimError> {
        let operation = snapshot.end_operation().ok_or(DbError::RecoveryConflict)?;
        let next_epoch = snapshot
            .owner_epoch()
            .checked_add(1)
            .ok_or(DbError::RecoveryConflict)?;
        let resource = self.resource_id()?;
        if snapshot.send_started()
            || snapshot.owner_instance_id() == manager.owner_instance_id()
            || !authorized()
        {
            return Err(DbError::RecoveryConflict.into());
        }
        let state = snapshot
            .query_original_tc(manager, business_intent)
            .await
            .map_err(|error| MySqlLauncherClaimError::Query(Box::new(error)))?
            .status();
        if state != GlobalStatus::Begin {
            return Err(MySqlLauncherClaimError::Observed(state));
        }
        let deadline = Instant::now() + self.data_source.operation_timeout();
        timeout_at(deadline, async {
            let mut connection = self.data_source.control_pool().acquire().await
                .map_err(|error| DbError::acquisition("TM launcher claim acquire", error))?;
            connection.close_on_drop();
            let mut transaction = connection.begin().await
                .map_err(|error| DbError::driver("TM launcher claim begin", error))?;
            let digest = Sha256::digest(snapshot.xid().as_str().as_bytes());
            // 先取得行锁，再用新语句的数据库时钟检查期限，避免等待行锁期间过期。
            let locked = sqlx::query("SELECT xid_digest FROM seata_rs_tm_launcher_v1 WHERE xid_digest=? FOR UPDATE")
                .bind(digest.as_slice())
                .fetch_optional(&mut *transaction).await
                .map_err(|error| DbError::driver("TM launcher claim lock", error))?;
            // 持锁后复验当前信任清单及挑战期限，阻止等待状态查询或行锁期间已退役的密钥。
            if locked.is_none() || !authorized() {
                return Err(DbError::RecoveryConflict);
            }
            let statement = Self::trust_guarded_sql("UPDATE seata_rs_tm_launcher_v1 SET owner_epoch=?,owner_instance_id=?,owner_stop_scope=?,modified_at=UTC_TIMESTAMP(6) WHERE xid_digest=? AND xid=? AND schema_version=1 AND tc_endpoint=? AND application_id=? AND service_group=? AND business_intent=? AND begin_exception_code=? AND begin_extra_data <=> ? AND owner_instance_id=? AND owner_stop_scope <=> ? AND owner_epoch=? AND end_operation=? AND send_started=0 AND deadline_at>UTC_TIMESTAMP(6)", snapshot.peer_trust_binding());
            let mut query = sqlx::query(&statement)
                .bind(next_epoch)
                .bind(manager.owner_instance_id().as_slice())
                .bind(manager.owner_stop_scope().map(|scope| scope.as_slice()))
                .bind(digest.as_slice())
                .bind(snapshot.xid().as_str().as_bytes())
                .bind(snapshot.tc_endpoint().to_string().into_bytes())
                .bind(snapshot.application_id().as_bytes())
                .bind(snapshot.service_group().as_bytes())
                .bind(snapshot.business_intent().as_bytes())
                .bind(snapshot.begin_exception_code().to_java_v1())
                .bind(snapshot.begin_extra_data().map(str::as_bytes))
                .bind(snapshot.owner_instance_id().as_slice())
                .bind(snapshot.owner_stop_scope().map(|scope| scope.as_slice()))
                .bind(snapshot.owner_epoch())
                .bind(Self::operation_code(operation))
;
            if let Some(binding) = snapshot.peer_trust_binding() { query = query.bind(binding.as_slice()); }
            let result = query.execute(&mut *transaction).await
                .map_err(|error| DbError::driver("TM launcher claim update", error))?;
            if result.rows_affected() != 1 {
                return Err(DbError::RecoveryConflict);
            }
            transaction.commit().await.map_err(|error| DbError::driver("TM launcher claim commit", error))
        }).await.map_err(|_| DbError::Deadline { operation: "TM launcher claim" })??;
        Ok(MySqlTransferredLauncher {
            snapshot,
            manager,
            epoch: next_epoch,
            operation,
            resource,
        })
    }

    pub(crate) fn resource_id(&self) -> Result<String, DbError> {
        self.data_source.resource_id()
    }

    pub(crate) async fn mark_transferred_send_started(
        &self,
        snapshot: &MySqlLauncherSnapshot,
        epoch: u64,
        manager: &TransactionManager,
        operation: TmOperation,
    ) -> Result<(), DbError> {
        if snapshot.peer_trust_binding() != manager.peer_trust_binding() {
            return Err(DbError::RecoveryConflict);
        }
        let deadline = Instant::now() + self.data_source.operation_timeout();
        timeout_at(deadline, async {
            let mut connection = self.data_source.control_pool().acquire().await
                .map_err(|error| DbError::acquisition("TM recovered send acquire", error))?;
            connection.close_on_drop();
            let mut transaction = connection.begin().await
                .map_err(|error| DbError::driver("TM recovered send begin", error))?;
            let digest = Sha256::digest(snapshot.xid().as_str().as_bytes());
            let locked = sqlx::query("SELECT xid_digest FROM seata_rs_tm_launcher_v1 WHERE xid_digest=? FOR UPDATE")
                .bind(digest.as_slice())
                .fetch_optional(&mut *transaction).await
                .map_err(|error| DbError::driver("TM recovered send lock", error))?;
            if locked.is_none() {
                return Err(DbError::RecoveryConflict);
            }
            // 新语句在行锁取得后检查服务器期限，并仅允许确认过的新代际置位一次。
            let statement = Self::trust_guarded_sql("UPDATE seata_rs_tm_launcher_v1 SET send_started=1,modified_at=UTC_TIMESTAMP(6) WHERE xid_digest=? AND xid=? AND schema_version=1 AND tc_endpoint=? AND application_id=? AND service_group=? AND business_intent=? AND begin_exception_code=? AND begin_extra_data <=> ? AND owner_instance_id=? AND owner_stop_scope <=> ? AND owner_epoch=? AND end_operation=? AND send_started=0 AND deadline_at>UTC_TIMESTAMP(6)", snapshot.peer_trust_binding());
            let mut query = sqlx::query(&statement)
                .bind(digest.as_slice())
                .bind(snapshot.xid().as_str().as_bytes())
                .bind(snapshot.tc_endpoint().to_string().into_bytes())
                .bind(snapshot.application_id().as_bytes())
                .bind(snapshot.service_group().as_bytes())
                .bind(snapshot.business_intent().as_bytes())
                .bind(snapshot.begin_exception_code().to_java_v1())
                .bind(snapshot.begin_extra_data().map(str::as_bytes))
                .bind(manager.owner_instance_id().as_slice())
                .bind(manager.owner_stop_scope().map(|scope| scope.as_slice()))
                .bind(epoch)
                .bind(Self::operation_code(operation))
;
            if let Some(binding) = snapshot.peer_trust_binding() { query = query.bind(binding.as_slice()); }
            let result = query.execute(&mut *transaction).await
                .map_err(|error| DbError::driver("TM recovered send update", error))?;
            if result.rows_affected() != 1 {
                return Err(DbError::RecoveryConflict);
            }
            transaction.commit().await.map_err(|error| DbError::driver("TM recovered send commit", error))
        }).await.map_err(|_| DbError::Deadline { operation: "TM recovered send" })?
    }
    /// 按 XID 摘要读取并核对完整原文、身份及状态；返回只读快照，不授予结束发送权。
    /// 不存在返回 None，摘要相同但原文不同或字段损坏时返回冲突。
    pub async fn load_snapshot(&self, xid: &Xid) -> Result<Option<MySqlLauncherSnapshot>, DbError> {
        let digest = Sha256::digest(xid.as_str().as_bytes());
        let deadline = Instant::now() + self.data_source.operation_timeout();
        let row = timeout_at(deadline, async {
            let mut connection = self.data_source.control_pool().acquire().await.map_err(|e| DbError::acquisition("TM launcher inspect acquire", e))?;
            connection.close_on_drop();
            sqlx::query("SELECT *,CAST(deadline_at <= UTC_TIMESTAMP(6) AS UNSIGNED) AS deadline_passed FROM seata_rs_tm_launcher_v1 WHERE xid_digest=?")
                .bind(digest.as_slice())
                .fetch_optional(&mut *connection).await.map_err(|e| DbError::driver("TM launcher inspect", e))
        }).await.map_err(|_| DbError::Deadline { operation: "TM launcher inspect" })??;
        let Some(row) = row else {
            return Ok(None);
        };
        let read = |column| DbError::driver("TM launcher inspect decode", column);
        let stored_xid: Vec<u8> = row.try_get("xid").map_err(read)?;
        if stored_xid != xid.as_str().as_bytes() {
            return Err(DbError::RecoveryConflict);
        }
        let version: u16 = row.try_get("schema_version").map_err(read)?;
        // 同一行读取版本与可选新列；只有版本 1 才允许旧 schema 缺少该列。
        let peer_trust_binding = match row.try_get::<Option<Vec<u8>>, _>("transport_binding") {
            Ok(Some(bytes)) if version == 2 => {
                Some(<[u8; 32]>::try_from(bytes).map_err(|_| DbError::RecoveryConflict)?)
            }
            Ok(None) if version == 1 => None,
            Err(sqlx::Error::ColumnNotFound(_)) if version == 1 => None,
            _ => return Err(DbError::RecoveryConflict),
        };
        let endpoint: Vec<u8> = row.try_get("tc_endpoint").map_err(read)?;
        let application: Vec<u8> = row.try_get("application_id").map_err(read)?;
        let group: Vec<u8> = row.try_get("service_group").map_err(read)?;
        let code: u8 = row.try_get("begin_exception_code").map_err(read)?;
        let extra: Option<Vec<u8>> = row.try_get("begin_extra_data").map_err(read)?;
        let intent: Vec<u8> = row.try_get("business_intent").map_err(read)?;
        let epoch: u64 = row.try_get("owner_epoch").map_err(read)?;
        let owner_instance: Option<Vec<u8>> = row.try_get("owner_instance_id").map_err(read)?;
        let owner_instance_id: [u8; 32] = owner_instance
            .ok_or(DbError::RecoveryConflict)?
            .try_into()
            .map_err(|_| DbError::RecoveryConflict)?;
        let scope: Option<Vec<u8>> = row.try_get("owner_stop_scope").map_err(read)?;
        let owner_stop_scope = scope
            .map(<[u8; 32]>::try_from)
            .transpose()
            .map_err(|_| DbError::RecoveryConflict)?;
        let operation: u8 = row.try_get("end_operation").map_err(read)?;
        let send: u8 = row.try_get("send_started").map_err(read)?;
        let expired: u64 = row.try_get("deadline_passed").map_err(read)?;
        if !matches!(version, 1 | 2)
            || epoch == 0
            || send > 1
            || expired > 1
            || (send == 1 && operation == 0)
        {
            return Err(DbError::RecoveryConflict);
        }
        let end_operation = match operation {
            0 => None,
            1 => Some(TmOperation::Commit),
            2 => Some(TmOperation::Rollback),
            _ => return Err(DbError::RecoveryConflict),
        };
        let tc_endpoint: std::net::SocketAddr = std::str::from_utf8(&endpoint)
            .ok()
            .and_then(|value| value.parse().ok())
            .ok_or(DbError::RecoveryConflict)?;
        let application_id =
            String::from_utf8(application).map_err(|_| DbError::RecoveryConflict)?;
        let service_group = String::from_utf8(group).map_err(|_| DbError::RecoveryConflict)?;
        let begin_extra_data = extra
            .map(String::from_utf8)
            .transpose()
            .map_err(|_| DbError::RecoveryConflict)?;
        let business_intent = String::from_utf8(intent).map_err(|_| DbError::RecoveryConflict)?;
        let begin_exception_code =
            TransactionExceptionCode::from_java_v1(code).map_err(|_| DbError::RecoveryConflict)?;
        if tc_endpoint.port() == 0
            || tc_endpoint.ip().is_unspecified()
            || application_id.trim().is_empty()
            || service_group.trim().is_empty()
            || business_intent.is_empty()
            || business_intent.len() > 256
        {
            return Err(DbError::RecoveryConflict);
        }
        Ok(Some(MySqlLauncherSnapshot {
            xid: xid.clone(),
            tc_endpoint,
            peer_trust_binding,
            application_id,
            service_group,
            begin_exception_code,
            begin_extra_data,
            business_intent,
            owner_epoch: epoch,
            owner_instance_id,
            owner_stop_scope,
            end_operation,
            send_started: send == 1,
            deadline_passed: expired == 1,
        }))
    }
    fn trust_guarded_sql(statement: &str, binding: Option<&[u8; 32]>) -> String {
        if binding.is_some() {
            format!(
                "{} AND transport_binding=?",
                statement.replace("schema_version=1", "schema_version=2")
            )
        } else {
            statement.to_owned()
        }
    }
    fn digest(origin: &LauncherOrigin) -> [u8; 32] {
        Sha256::digest(origin.xid().as_str().as_bytes()).into()
    }
    fn operation_code(operation: TmOperation) -> u8 {
        match operation {
            TmOperation::Commit => 1,
            TmOperation::Rollback => 2,
            _ => 0,
        }
    }
    async fn update(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
        send: bool,
    ) -> Result<(), DbError> {
        let code = Self::operation_code(operation);
        if code == 0 {
            return Err(DbError::Input {
                reason: "commit or rollback required",
            });
        }
        let deadline = Instant::now() + self.data_source.operation_timeout();
        let result = timeout_at(deadline, async {
            let mut connection = self.data_source.control_pool().acquire().await.map_err(|e| DbError::acquisition("TM launcher acquire", e))?;
            connection.close_on_drop();
            let sql = if send {
                "UPDATE seata_rs_tm_launcher_v1 SET send_started=1,modified_at=UTC_TIMESTAMP(6) WHERE xid_digest=? AND xid=? AND tc_endpoint=? AND application_id=? AND service_group=? AND business_intent=? AND owner_instance_id=? AND owner_stop_scope <=> ? AND owner_epoch=1 AND end_operation=? AND send_started=0 AND schema_version=1"
            } else {
                "UPDATE seata_rs_tm_launcher_v1 SET end_operation=?,modified_at=UTC_TIMESTAMP(6) WHERE xid_digest=? AND xid=? AND tc_endpoint=? AND application_id=? AND service_group=? AND business_intent=? AND owner_instance_id=? AND owner_stop_scope <=> ? AND owner_epoch=1 AND end_operation=0 AND send_started=0 AND schema_version=1"
            };
            let statement = Self::trust_guarded_sql(sql, origin.peer_trust_binding());
            let query = sqlx::query(&statement);
            let query = if send { query } else { query.bind(code) };
            let digest = Self::digest(origin);
            let affected = query
                .bind(digest.as_slice())
                .bind(origin.xid().as_str().as_bytes())
                .bind(origin.tc_endpoint().to_string().into_bytes())
                .bind(origin.application_id().as_bytes())
                .bind(origin.service_group().as_bytes())
                .bind(origin.business_intent().as_bytes())
                .bind(origin.owner_instance_id().as_slice())
                .bind(origin.owner_stop_scope().map(|scope| scope.as_slice()));
            let affected = if send { affected.bind(code) } else { affected };
            let affected = if let Some(binding) = origin.peer_trust_binding() { affected.bind(binding.as_slice()) } else { affected };
            affected.execute(&mut *connection).await.map_err(|e| DbError::driver("TM launcher update", e)).map(|v| v.rows_affected())
        }).await.map_err(|_| DbError::Deadline { operation: "TM launcher update" })??;
        if result != 1 {
            return Err(DbError::RecoveryConflict);
        }
        Ok(())
    }
}
impl LauncherStore for MySqlLauncherStore {
    async fn record_begin(
        &self,
        origin: &LauncherOrigin,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let deadline = Instant::now() + self.data_source.operation_timeout();
        let result = timeout_at(deadline, async {
            let mut connection = self.data_source.control_pool().acquire().await.map_err(|e| DbError::acquisition("TM launcher acquire", e))?;
            connection.close_on_drop();
            // 扣除本控制操作的最大预算，使数据库求出的截止时间不晚于原 TC 期限。
            let safe_remaining = origin.remaining().saturating_sub(self.data_source.operation_timeout());
            let remaining = i64::try_from(safe_remaining.as_micros()).map_err(|_| DbError::Invalid { reason: "TM remaining deadline exceeds MySQL interval" })?;
            if remaining <= 0 { return Err(DbError::Deadline { operation: "TM launcher insert" }); }
            let plain_statement = "INSERT INTO seata_rs_tm_launcher_v1 (schema_version,xid_digest,xid,tc_endpoint,application_id,service_group,begin_exception_code,begin_extra_data,business_intent,owner_instance_id,owner_stop_scope,deadline_at,created_at,modified_at) VALUES (1,?,?,?,?,?,?,?,?,?,?,DATE_ADD(UTC_TIMESTAMP(6),INTERVAL ? MICROSECOND),UTC_TIMESTAMP(6),UTC_TIMESTAMP(6))";
            let statement = if origin.peer_trust_binding().is_some() {
                plain_statement.replace("owner_stop_scope,deadline_at", "owner_stop_scope,transport_binding,deadline_at")
                    .replace("VALUES (1,", "VALUES (2,").replace("?,DATE_ADD", "?,?,DATE_ADD")
            } else { plain_statement.to_owned() };
            let digest = Self::digest(origin);
            let mut query = sqlx::query(&statement)
                .bind(digest.as_slice())
                .bind(origin.xid().as_str().as_bytes())
                .bind(origin.tc_endpoint().to_string().into_bytes())
                .bind(origin.application_id().as_bytes())
                .bind(origin.service_group().as_bytes())
                .bind(origin.begin_exception_code().to_java_v1())
                .bind(origin.begin_extra_data().map(str::as_bytes))
                .bind(origin.business_intent().as_bytes())
                .bind(origin.owner_instance_id().as_slice())
                .bind(origin.owner_stop_scope().map(|scope| scope.as_slice()))
;
            if let Some(binding) = origin.peer_trust_binding() { query = query.bind(binding.as_slice()); }
            query.bind(remaining)
                .execute(&mut *connection).await.map_err(|e| DbError::driver("TM launcher insert", e)).map(|v| v.rows_affected())
        }).await.map_err(|_| DbError::Deadline { operation: "TM launcher insert" })??;
        if result != 1 {
            return Err(Box::new(DbError::RecoveryConflict));
        }
        Ok(())
    }
    async fn record_end_intent(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.update(origin, operation, false)
            .await
            .map_err(Into::into)
    }
    async fn mark_send_started(
        &self,
        origin: &LauncherOrigin,
        operation: TmOperation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.update(origin, operation, true)
            .await
            .map_err(Into::into)
    }
}
