use crate::{DbError, MySqlLocalTransaction, MySqlOptions};
use sqlx::{mysql::MySqlPoolOptions, MySqlConnection, MySqlPool, Row};
use std::{fmt, time::Duration};
use tokio::time::{timeout_at, Instant};

/// 拥有SQLx连接池的MySQL数据源，连接建立后验证真实会话；提供有界本地事务与独立Undo控制事务。
/// 对应 Java: org.apache.seata.rm.datasource.DataSourceProxy的MySQL基础路径（部分实现）。
#[derive(Clone)]
pub struct MySqlDataSource {
    pool: MySqlPool,
    database: String,
    server_version: String,
    operation_timeout: Duration,
}
impl MySqlDataSource {
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
        use crate::UndoRollbackError;
        if xid.is_empty() || xid.len() > 128 || branch_id <= 0 {
            return Err(UndoRollbackError::InvalidIdentity);
        }
        let deadline = Instant::now() + self.operation_timeout;
        let mut commit_attempted = false;
        let result = timeout_at(deadline, async {
            let mut connection = self
                .pool
                .acquire()
                .await
                .map_err(|e| DbError::driver("Undo rollback acquire", e))?;
            connection.close_on_drop();
            let mut tx = sqlx::Transaction::begin(connection, None)
                .await
                .map_err(|e| DbError::driver("Undo rollback begin", e))?;
            let (observation, commit) = crate::undo::AbstractUndoLogManager::prepare(
                &mut tx,
                &self.database,
                xid,
                branch_id,
                profile,
            )
            .await?;
            if Instant::now() >= deadline {
                return Err(DbError::Deadline {
                    operation: "Undo rollback",
                });
            }
            if commit {
                commit_attempted = true;
                tx.commit()
                    .await
                    .map_err(|e| DbError::driver("Undo rollback commit", e))?;
            } else {
                tx.rollback()
                    .await
                    .map_err(|e| DbError::driver("Undo rollback read rollback", e))?;
            }
            Ok(observation)
        })
        .await;
        let result = if Instant::now() >= deadline {
            Err(DbError::Deadline {
                operation: "Undo rollback",
            })
        } else {
            result.unwrap_or(Err(DbError::Deadline {
                operation: "Undo rollback",
            }))
        };
        result.map_err(|source| {
            if commit_attempted {
                UndoRollbackError::CommitOutcomeUnknown {
                    xid: xid.into(),
                    branch_id,
                    source,
                }
            } else {
                UndoRollbackError::BeforeCommit {
                    xid: xid.into(),
                    branch_id,
                    source,
                }
            }
        })
    }
    /// 参数为原XID/分支号；确认独立控制围栏，不提交业务或证明二阶段结果。
    pub async fn fence_undo_log(
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
            let decision =
                MySqlUndoLogManager::prepare_fence(&mut tx, &self.database, xid, branch_id).await?;
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
        let pool=timeout_at(deadline,MySqlPoolOptions::new().max_connections(options.maximum_connections).min_connections(0).acquire_timeout(options.acquire_timeout)
            .after_connect(|connection,_|Box::pin(async move {
                sqlx::query("SET SESSION sql_mode='STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION,NO_BACKSLASH_ESCAPES', transaction_isolation='REPEATABLE-READ', innodb_lock_wait_timeout=2, lock_wait_timeout=2, max_execution_time=2000")
                    .execute(connection).await?;
                Ok(())
            })).connect_with(options.connect_options())).await.map_err(|_|DbError::Deadline { operation: "connect" })?
            .map_err(|error|DbError::driver("connect",error))?;
        let mut connection = timeout_at(deadline, pool.acquire())
            .await
            .map_err(|_| DbError::Deadline {
                operation: "connect",
            })?
            .map_err(|error| DbError::driver("profile acquire", error))?;
        connection.close_on_drop();
        let result = timeout_at(deadline, Self::profile(&mut connection, &options.database))
            .await
            .map_err(|_| DbError::Deadline {
                operation: "connect",
            })?;
        drop(connection);
        match result {
            Ok(server_version) => Ok(Self {
                pool,
                database: options.database,
                server_version,
                operation_timeout: options.operation_timeout,
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
