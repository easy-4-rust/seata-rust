use crate::mysql_at_registration_checks::MySqlAtRegistrationChecks;
use crate::{DbError, MySqlLocalTransaction, MySqlOptions};
use sqlx::{mysql::MySqlPoolOptions, MySqlConnection, MySqlPool, Row};
use std::{
    fmt,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::time::{timeout_at, Instant};

/// 拥有SQLx连接池的MySQL数据源，连接建立后验证真实会话；提供有界本地事务与独立Undo控制事务。
/// 对应 Java: org.apache.seata.rm.datasource.DataSourceProxy的MySQL基础路径（部分实现）。
#[derive(Clone)]
pub struct MySqlDataSource {
    pool: MySqlPool,
    database: String,
    server_version: String,
    pub(crate) server_uuid: Arc<str>,
    pub(crate) registration_checks: MySqlAtRegistrationChecks,
    operation_timeout: Duration,
    recovery_metrics: crate::MySqlRecoveryMetricsSampler,
    undo_metrics: crate::MySqlUndoMetricsSampler,
    phase_one_metrics: crate::MySqlPhaseOneMetricsSampler,
}
impl MySqlDataSource {
    /// 无参数；返回同数据源克隆共享的一阶段编排计数，不授予事务或报告权限。
    pub fn phase_one_metrics_sampler(&self) -> crate::MySqlPhaseOneMetricsSampler {
        self.phase_one_metrics.clone()
    }

    /// 无参数；返回同数据源克隆共享的独立 Undo 调用计数，不授予连接、恢复或关闭权限。
    pub fn undo_metrics_sampler(&self) -> crate::MySqlUndoMetricsSampler {
        self.undo_metrics.clone()
    }

    /// 无参数；返回同数据源克隆与管理器共享的只读控制事务计数，不授予数据库权限。
    pub fn recovery_metrics_sampler(&self) -> crate::MySqlRecoveryMetricsSampler {
        self.recovery_metrics.clone()
    }

    /// 无参数；返回本单主机配置对应的原 Java 普通 MySQL JDBC 资源标识，不含认证或查询参数。
    /// 不规范化主机大小写或 DNS 别名；负载均衡 URL 尚不在此入口范围。
    pub fn resource_id(&self) -> Result<String, DbError> {
        let options = self.pool.connect_options();
        let host = options.get_host();
        let rendered = if host.parse::<std::net::Ipv6Addr>().is_ok() {
            format!("[{host}]")
        } else if !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        {
            host.to_owned()
        } else {
            return Err(DbError::Invalid {
                reason: "unambiguous single-host JDBC resource required",
            });
        };
        Ok(format!(
            "jdbc:mysql://{rendered}:{}/{}",
            options.get_port(),
            self.database
        ))
    }
    /// 无参数；返回内部控制操作共享池，所有调用者必须在首个事务 await 前设置关闭策略。
    pub(crate) fn control_pool(&self) -> &MySqlPool {
        &self.pool
    }
    /// 无参数；返回连接建立时已核对的原 schema。
    pub(crate) fn database(&self) -> &str {
        &self.database
    }
    /// 无参数；返回完整控制操作的绝对期限预算。
    pub(crate) fn operation_timeout(&self) -> Duration {
        self.operation_timeout
    }
    /// 参数为原XID/分支与固定JDBC配置；返回真实Undo处理事实，不提交原业务或证明TC所有权。
    /// 对应 Java: AbstractUndoLogManager#undo的MySQL路径（部分实现）。
    pub async fn rollback_undo_log(
        &self,
        xid: &str,
        branch_id: i64,
        profile: seata_at::MySqlJdbcProfile,
    ) -> Result<crate::UndoRollbackObservation, crate::UndoRollbackError> {
        self.rollback_undo_log_for_serializer(
            xid,
            branch_id,
            profile,
            crate::undo::UndoSerializer::Jackson,
        )
        .await
    }

    async fn rollback_undo_log_for_serializer(
        &self,
        xid: &str,
        branch_id: i64,
        profile: seata_at::MySqlJdbcProfile,
        format: crate::undo::UndoSerializer,
    ) -> Result<crate::UndoRollbackObservation, crate::UndoRollbackError> {
        let guard = self.undo_metrics.begin(crate::MySqlUndoOperation::Rollback);
        let result = self
            .rollback_undo_log_for_serializer_inner(xid, branch_id, profile, format)
            .await;
        guard.finish_rollback(&result);
        result
    }

    async fn rollback_undo_log_for_serializer_inner(
        &self,
        xid: &str,
        branch_id: i64,
        profile: seata_at::MySqlJdbcProfile,
        format: crate::undo::UndoSerializer,
    ) -> Result<crate::UndoRollbackObservation, crate::UndoRollbackError> {
        use crate::UndoRollbackError;
        if xid.is_empty() || xid.len() > 128 || branch_id <= 0 {
            return Err(UndoRollbackError::InvalidIdentity);
        }
        let deadline = Instant::now() + self.operation_timeout;
        let mut attempt = crate::undo_rollback_attempt::UndoRollbackAttempt::default();
        let result = timeout_at(deadline, async {
            let mut connection = self
                .pool
                .acquire()
                .await
                .map_err(|source| DbError::acquisition("Undo rollback acquire", source))?;
            connection.close_on_drop();
            let mut tx = sqlx::Transaction::begin(connection, None)
                .await
                .map_err(|e| DbError::driver("Undo rollback begin", e))?;
            let (observation, commit) =
                match crate::undo::AbstractUndoLogManager::prepare_for_serializer(
                    &mut tx,
                    &self.pool.connect_options(),
                    &self.database,
                    xid,
                    branch_id,
                    profile,
                    format,
                )
                .await
                {
                    Ok(prepared) => prepared,
                    Err(source) => {
                        attempt.abort(source, tx.rollback()).await;
                        return Ok(None);
                    }
                };
            if Instant::now() >= deadline {
                return Err(DbError::Deadline {
                    operation: "Undo rollback",
                });
            }
            if commit {
                attempt.start_commit();
                tx.commit()
                    .await
                    .map_err(|e| DbError::driver("Undo rollback commit", e))?;
            } else {
                tx.rollback()
                    .await
                    .map_err(|e| DbError::driver("Undo rollback read rollback", e))?;
            }
            Ok(Some(observation))
        })
        .await;
        attempt.finish(xid, branch_id, result.ok(), Instant::now() >= deadline)
    }
    /// 参数为原XID/分支号；确认独立控制围栏，不提交业务或证明二阶段结果。
    pub async fn fence_undo_log(
        &self,
        xid: &str,
        branch_id: i64,
    ) -> Result<crate::UndoFenceObservation, crate::UndoFenceError> {
        let guard = self.undo_metrics.begin(crate::MySqlUndoOperation::Fence);
        let result = self.fence_undo_log_inner(xid, branch_id).await;
        guard.finish_fence(&result);
        result
    }

    async fn fence_undo_log_inner(
        &self,
        xid: &str,
        branch_id: i64,
    ) -> Result<crate::UndoFenceObservation, crate::UndoFenceError> {
        use crate::mysql::{MySqlUndoLogManager, UndoFenceDecision};
        use crate::{UndoFenceError, UndoFenceObservation};
        if xid.is_empty() || xid.len() > 128 || branch_id <= 0 {
            return Err(UndoFenceError::InvalidIdentity);
        }
        let deadline = Instant::now() + self.operation_timeout;
        let mut commit_attempted = false;
        let result = timeout_at(deadline, async {
            let mut connection = self
                .pool
                .acquire()
                .await
                .map_err(|e| DbError::driver("Undo fence acquire", e))?;
            // 首次事务await前确定关闭策略；取消任何阶段都不会返还半协议/脏连接。
            connection.close_on_drop();
            let mut tx = sqlx::Transaction::begin(connection, None)
                .await
                .map_err(|e| DbError::driver("Undo fence begin", e))?;
            let decision = MySqlUndoLogManager::prepare_fence(
                &mut tx,
                &self.pool.connect_options(),
                &self.database,
                xid,
                branch_id,
            )
            .await?;
            if Instant::now() >= deadline {
                return Err(DbError::Deadline {
                    operation: "Undo fence",
                });
            }
            let observation = match decision {
                UndoFenceDecision::InsertedGuard => {
                    commit_attempted = true;
                    tx.commit()
                        .await
                        .map_err(|e| DbError::driver("Undo fence commit", e))?;
                    UndoFenceObservation::Fenced { inserted: true }
                }
                UndoFenceDecision::ExistingGuard | UndoFenceDecision::NormalRow => {
                    tx.rollback()
                        .await
                        .map_err(|e| DbError::driver("Undo fence read rollback", e))?;
                    match decision {
                        UndoFenceDecision::ExistingGuard => {
                            UndoFenceObservation::Fenced { inserted: false }
                        }
                        _ => UndoFenceObservation::UndoPresent,
                    }
                }
            };
            Ok(observation)
        })
        .await;
        // 即使驱动已Ready，也不能越过同一绝对期限发布控制成功。
        let result = if Instant::now() >= deadline {
            Err(DbError::Deadline {
                operation: "Undo fence",
            })
        } else {
            result.unwrap_or(Err(DbError::Deadline {
                operation: "Undo fence",
            }))
        };
        result.map_err(|source| {
            if commit_attempted {
                UndoFenceError::CommitOutcomeUnknown {
                    xid: xid.into(),
                    branch_id,
                    source,
                }
            } else {
                UndoFenceError::BeforeCommit {
                    xid: xid.into(),
                    branch_id,
                    source,
                }
            }
        })
    }
    /// 参数为有限连接/TLS配置；返回真实连接且已核实会话的源，未认证配置不自动回退。
    pub async fn connect(options: MySqlOptions) -> Result<Self, DbError> {
        let deadline = Instant::now() + options.operation_timeout;
        let identity = Arc::new(OnceLock::<Arc<str>>::new());
        let pool_identity = identity.clone();
        let pool_database = options.database.clone();
        let pool_options = MySqlPoolOptions::new()
            .max_connections(options.maximum_connections)
            .min_connections(0)
            .acquire_timeout(options.acquire_timeout)
            .after_connect(move |connection, _| {
                let identity = pool_identity.clone();
                let database = pool_database.clone();
                Box::pin(async move {
                    sqlx::query(concat!(
                        "SET SESSION sql_mode='STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION,NO_BACKSLASH_ESCAPES', ",
                        "transaction_isolation='REPEATABLE-READ', innodb_lock_wait_timeout=2, ",
                        "lock_wait_timeout=2, max_execution_time=2000",
                    ))
                    .execute(&mut *connection)
                    .await?;
                    if let Some(expected) = identity.get() {
                        let (uuid, schema): (String, String) =
                            sqlx::query_as("SELECT @@server_uuid,DATABASE()")
                                .fetch_one(connection)
                                .await?;
                        if uuid.as_str() != expected.as_ref() || schema != database {
                            return Err(sqlx::Error::Protocol(
                                "MySQL pool requires the original physical schema".into(),
                            ));
                        }
                    }
                    Ok(())
                })
            });
        let pool = timeout_at(
            deadline,
            pool_options.connect_with(options.connect_options()),
        )
        .await
        .map_err(|_| DbError::Deadline {
            operation: "connect",
        })?
        .map_err(|error| DbError::driver("connect", error))?;
        let mut connection = timeout_at(deadline, pool.acquire())
            .await
            .map_err(|_| DbError::Deadline {
                operation: "connect",
            })?
            .map_err(|error| DbError::driver("profile acquire", error))?;
        connection.close_on_drop();
        // 连接池尚未发布；在通过 MySQL 会话校验的同一连接上固定后续连接的身份。
        let result = timeout_at(deadline, async {
            let version = Self::profile(&mut connection, &options.database).await?;
            let uuid: String = sqlx::query_scalar("SELECT @@server_uuid")
                .fetch_one(&mut *connection)
                .await
                .map_err(|error| DbError::driver("profile server UUID", error))?;
            if uuid.is_empty() {
                return Err(DbError::Unsupported {
                    feature: "empty MySQL server UUID",
                });
            }
            let uuid: Arc<str> = uuid.into();
            identity
                .set(uuid.clone())
                .map_err(|_| DbError::RecoveryConflict)?;
            Ok((version, uuid))
        })
        .await
        .map_err(|_| DbError::Deadline {
            operation: "connect",
        })?;
        drop(connection);
        match result {
            Ok((server_version, server_uuid)) => Ok(Self {
                pool,
                database: options.database,
                server_version,
                server_uuid,
                registration_checks: MySqlAtRegistrationChecks {
                    require_resource_binding: options.require_resource_binding,
                    require_tinyint_profile_binding: options.require_tinyint_profile_binding,
                    java_tc_mysql_lock_limits: options.java_tc_mysql_lock_limits,
                    java_tc_mysql_lock_collation: options.java_tc_mysql_lock_collation,
                },
                operation_timeout: options.operation_timeout,
                recovery_metrics: crate::MySqlRecoveryMetricsSampler::new(),
                undo_metrics: crate::MySqlUndoMetricsSampler::new(),
                phase_one_metrics: crate::MySqlPhaseOneMetricsSampler::new(),
            }),
            Err(error) => {
                // 最后Pool owner丢弃，所有租约已标记有限关闭，不在失败路径再做无界await。
                drop(pool);
                Err(error)
            }
        }
    }
    async fn profile(connection: &mut MySqlConnection, database: &str) -> Result<String, DbError> {
        let row=sqlx::query("SELECT VERSION() AS version, DATABASE() AS db, @@transaction_isolation AS isolation_level, @@sql_mode AS sql_mode, @@character_set_connection AS charset")
            .fetch_one(connection).await.map_err(|error|DbError::driver("session profile",error))?;
        let version: String = row
            .try_get("version")
            .map_err(|error| DbError::driver("profile version", error))?;
        let db: String = row
            .try_get("db")
            .map_err(|error| DbError::driver("profile database", error))?;
        let isolation: String = row
            .try_get("isolation_level")
            .map_err(|error| DbError::driver("profile isolation", error))?;
        let mode: String = row
            .try_get("sql_mode")
            .map_err(|error| DbError::driver("profile mode", error))?;
        let charset: String = row
            .try_get("charset")
            .map_err(|error| DbError::driver("profile charset", error))?;
        if !version.starts_with("8.")
            || db != database
            || isolation != "REPEATABLE-READ"
            || charset != "utf8mb4"
            || !mode.split(',').any(|value| value == "NO_BACKSLASH_ESCAPES")
            || !mode.split(',').any(|value| value == "STRICT_ALL_TABLES")
        {
            return Err(DbError::Unsupported {
                feature: "server/session profile",
            });
        }
        Ok(version)
    }
    /// 无参数；返回现场读取的原服务器版本，不代表其他MySQL版本已认证。
    pub fn server_version(&self) -> &str {
        &self.server_version
    }
    /// 无参数；开始独占连接的本地作用域，元数据锁一直持有到rollback/Drop。
    pub async fn begin(&self) -> Result<MySqlLocalTransaction, DbError> {
        let deadline = Instant::now() + self.operation_timeout;
        let mut connection = timeout_at(deadline, self.pool.acquire())
            .await
            .map_err(|_| DbError::Deadline { operation: "begin" })?
            .map_err(|error| DbError::driver("begin acquire", error))?;
        // 先设置关闭策略，再将租约交给SQLx拥有式事务，取消BEGIN也不能回收脏协议连接。
        connection.close_on_drop();
        let transaction = timeout_at(deadline, sqlx::Transaction::begin(connection, None))
            .await
            .map_err(|_| DbError::Deadline { operation: "begin" })?
            .map_err(|error| DbError::driver("begin", error))?;
        Ok(MySqlLocalTransaction::new(
            transaction,
            self.database.clone(),
            // 保留原低层 begin 行为；只有新增准备接口要求可精确构造 JDBC 资源名。
            self.resource_id().ok(),
            self.server_uuid.clone(),
            self.registration_checks,
            self.pool.connect_options(),
            self.operation_timeout,
        ))
    }
    /// 无参数；停止新借用并最多等待3秒关闭，超时不报告完整排空。
    pub async fn close(&self) -> Result<(), DbError> {
        tokio::time::timeout(Duration::from_secs(3), self.pool.close())
            .await
            .map_err(|_| DbError::Deadline {
                operation: "pool close",
            })
    }
}
impl fmt::Debug for MySqlDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlDataSource")
            .field("server_version", &self.server_version)
            .field("pool_closed", &self.pool.is_closed())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "data_source_proxy_fastjson_tests.rs"]
mod fastjson_tests;

#[cfg(test)]
#[path = "data_source_proxy_rollback_sqlx_tests.rs"]
pub(crate) mod rollback_sqlx_tests;
