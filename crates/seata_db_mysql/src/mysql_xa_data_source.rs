use crate::{DbError, MySqlDataSource, MySqlXaError, MySqlXaTransaction};
use futures_util::TryStreamExt;
use seata_xa::{XaBranchXid, XaOperation, XaXid};
use sqlx::{pool::PoolConnection, MySql, Row};
use tokio::time::{timeout_at, Instant};
/// 复用原 TLS/物理实例绑定的数据源，执行显式原身份 XA 命令及有界 Prepared 观察。
/// 对应 Java: org.apache.seata.rm.datasource.xa.DataSourceProxyXA 的 MySQL 原生路径（部分实现）。
#[derive(Clone, Debug)]
pub struct MySqlXaDataSource {
    pub(crate) source: MySqlDataSource,
}
impl MySqlXaDataSource {
    /// 参数为已通过 TLS/实例/schema 校验的原数据源；返回独立 XA 入口，不授予 TC 分支登记权。
    pub fn new(source: MySqlDataSource) -> Self {
        Self { source }
    }
    /// 参数为原字节身份；返回独占物理连接的 ACTIVE 作用域，禁止普通本地提交。
    pub async fn start(&self, xid: XaBranchXid) -> Result<MySqlXaTransaction, MySqlXaError> {
        let arguments = xid.mysql_arguments()?;
        let deadline = Instant::now() + self.source.operation_timeout();
        let mut connection = self.acquire(deadline, XaOperation::Start).await?;
        let detached: Result<i64, DbError> = timeout_at(deadline, async {
            sqlx::query_scalar("SELECT @@session.xa_detach_on_prepare")
                .fetch_one(&mut *connection)
                .await
                .map_err(|e| DbError::driver("XA detached profile", e))
        })
        .await
        .unwrap_or(Err(DbError::Deadline {
            operation: "XA profile",
        }));
        if detached.map_err(|source| MySqlXaError::NotSent {
            operation: XaOperation::Start,
            source,
        })? != 1
        {
            return Err(MySqlXaError::NotSent {
                operation: XaOperation::Start,
                source: DbError::Unsupported {
                    feature: "XA detached preparation required",
                },
            });
        }
        Self::command(
            &mut connection,
            &format!("XA START {arguments}"),
            deadline,
            XaOperation::Start,
        )
        .await?;
        Ok(MySqlXaTransaction::new(self.clone(), xid, connection))
    }
    /// 参数为原身份；成功仅确认本次 XA COMMIT 的真实响应，错误/取消不得盲重放或反推方向。
    pub async fn commit_prepared(&self, xid: &XaBranchXid) -> Result<(), MySqlXaError> {
        self.resolve(xid, XaOperation::Commit).await
    }
    /// 参数为原身份；成功仅确认本次 XA ROLLBACK 响应，XAER_NOTA 仍返回未知错误。
    pub async fn rollback_prepared(&self, xid: &XaBranchXid) -> Result<(), MySqlXaError> {
        self.resolve(xid, XaOperation::Rollback).await
    }
    async fn resolve(&self, xid: &XaBranchXid, operation: XaOperation) -> Result<(), MySqlXaError> {
        let arguments = xid.mysql_arguments()?;
        let deadline = Instant::now() + self.source.operation_timeout();
        let mut connection = self.acquire(deadline, operation).await?;
        let verb = if operation == XaOperation::Commit {
            "COMMIT"
        } else {
            "ROLLBACK"
        };
        Self::command(
            &mut connection,
            &format!("XA {verb} {arguments}"),
            deadline,
            operation,
        )
        .await
    }
    /// 参数为完整原身份；返回当前 XA RECOVER 是否精确可见 Prepared，不表示已提交/未提交或 TC 所有权。
    pub async fn prepared_observed(&self, xid: &XaBranchXid) -> Result<bool, MySqlXaError> {
        xid.mysql_arguments()?;
        let deadline = Instant::now() + self.source.operation_timeout();
        let mut connection = self.acquire(deadline, XaOperation::Recover).await?;
        let result = timeout_at(deadline, async {
            let mut rows = sqlx::raw_sql("XA RECOVER").fetch(&mut *connection);
            let mut count = 0;
            let mut found = false;
            while let Some(row) = rows
                .try_next()
                .await
                .map_err(|e| DbError::driver("XA recover", e))?
            {
                count += 1;
                if count > 256 {
                    return Err(DbError::Unsupported {
                        feature: "XA recover row budget",
                    });
                }
                let format: i32 = row
                    .try_get("formatID")
                    .map_err(|e| DbError::driver("XA recover format", e))?;
                if format != XaBranchXid::FORMAT_ID {
                    continue;
                }
                let global: i32 = row
                    .try_get("gtrid_length")
                    .map_err(|e| DbError::driver("XA recover global length", e))?;
                let branch: i32 = row
                    .try_get("bqual_length")
                    .map_err(|e| DbError::driver("XA recover branch length", e))?;
                let data: Vec<u8> = row
                    .try_get("data")
                    .map_err(|e| DbError::driver("XA recover data", e))?;
                if !(0..=64).contains(&global)
                    || !(0..=64).contains(&branch)
                    || data.len() != global as usize + branch as usize
                {
                    return Err(DbError::RecoveryConflict);
                }
                let matches = data[..global as usize] == *xid.get_global_transaction_id()
                    && data[global as usize..] == *xid.get_branch_qualifier();
                if matches {
                    if found {
                        return Err(DbError::RecoveryConflict);
                    }
                    found = true;
                }
            }
            Ok(found)
        })
        .await
        .unwrap_or(Err(DbError::Deadline {
            operation: "XA recover",
        }))
        .map_err(|source| MySqlXaError::Observation { source });
        if Instant::now() >= deadline {
            return Err(MySqlXaError::Observation {
                source: DbError::Deadline {
                    operation: "XA recover",
                },
            });
        }
        result
    }
    /// 参数为绝对期限及原阶段；返回一条已配置退出即关闭的独占连接，失败不发送该阶段。
    pub(crate) async fn acquire(
        &self,
        deadline: Instant,
        operation: XaOperation,
    ) -> Result<PoolConnection<MySql>, MySqlXaError> {
        let mut connection = timeout_at(deadline, self.source.control_pool().acquire())
            .await
            .map_err(|_| MySqlXaError::NotSent {
                operation,
                source: DbError::Deadline {
                    operation: "XA acquire",
                },
            })?
            .map_err(|e| MySqlXaError::NotSent {
                operation,
                source: DbError::acquisition("XA acquire", e),
            })?;
        // 原生 XA 不使用 SQLx 本地事务计数；任何退出都必须真正退休此连接，禁止脏会话回池。
        connection.close_on_drop();
        Ok(connection)
    }
    /// 参数为原独占连接、安全生成的固定控制 SQL、绝对期限和阶段；返回真实确认或脱敏未知。
    pub(crate) async fn command(
        connection: &mut PoolConnection<MySql>,
        sql: &str,
        deadline: Instant,
        operation: XaOperation,
    ) -> Result<(), MySqlXaError> {
        if Instant::now() >= deadline {
            return Err(MySqlXaError::NotSent {
                operation,
                source: DbError::Deadline {
                    operation: "XA before command",
                },
            });
        }
        // XA 控制使用 COM_QUERY；固定 MySQL 对 prepared-statement 协议返回 1295。
        // sql 仅来自固定动词、原字节十六进制和固定格式号，禁止拼接原业务文本。
        timeout_at(deadline, sqlx::raw_sql(sql).execute(&mut **connection))
            .await
            .map_err(|_| MySqlXaError::Unknown {
                operation,
                source: DbError::Deadline {
                    operation: "XA command",
                },
            })?
            .map_err(|e| MySqlXaError::Unknown {
                operation,
                source: DbError::driver("XA command", e),
            })?;
        if Instant::now() >= deadline {
            return Err(MySqlXaError::Unknown {
                operation,
                source: DbError::Deadline {
                    operation: "XA command",
                },
            });
        }
        Ok(())
    }
}
