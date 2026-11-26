use crate::{DbError, MySqlLocalTransaction, MySqlOptions};
use sqlx::{mysql::MySqlPoolOptions, MySqlConnection, MySqlPool, Row};
use std::{fmt, time::Duration};
use tokio::time::{timeout_at, Instant};

/// 拥有SQLx连接池的MySQL数据源，连接建立后验证真实会话；当前只暴露事务内元数据读取。
/// 对应 Java: org.apache.seata.rm.datasource.DataSourceProxy的MySQL基础路径（部分实现）。
#[derive(Clone)]
pub struct MySqlDataSource {
    pool: MySqlPool,
    database: String,
    server_version: String,
    operation_timeout: Duration,
}
impl MySqlDataSource {
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
