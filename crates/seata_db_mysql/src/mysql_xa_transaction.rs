use crate::{
    mysql_image_reader::MySqlImageReader, mysql_xa_table::MySqlXaTable, DbError, MySqlXaDataSource,
    MySqlXaError,
};
use seata_at::SqlRecognizer;
use seata_datasource::FieldValue;
use seata_xa::{XaBranchXid, XaOperation, XaXid};
use sqlx::{pool::PoolConnection, MySql};
use tokio::time::{timeout_at, Instant};
/// 独占连接的 ACTIVE XA 作用域；失败/取消后中毒，Prepare 消费原作用域。
/// 对应 Java: org.apache.seata.rm.datasource.xa.ConnectionProxyXA 的原生两阶段路径（部分实现）。
pub struct MySqlXaTransaction {
    source: MySqlXaDataSource,
    xid: XaBranchXid,
    connection: Option<PoolConnection<MySql>>,
    poisoned: bool,
}
impl MySqlXaTransaction {
    /// 参数为原数据源、原字节身份与已开始 XA 的独占连接；返回仅能消费式结束的活动作用域。
    pub(crate) fn new(
        source: MySqlXaDataSource,
        xid: XaBranchXid,
        connection: PoolConnection<MySql>,
    ) -> Self {
        Self {
            source,
            xid,
            connection: Some(connection),
            poisoned: false,
        }
    }
    /// 无参数；返回原字节标识的只读引用，不证明 TC 登记或全局完成。
    pub fn xid(&self) -> &XaBranchXid {
        &self.xid
    }
    /// 参数为原有界 DML 及完整参数；在原连接执行一次，失败或取消不允许重用活动作用域。
    pub async fn execute(
        &mut self,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<u64, MySqlXaError> {
        if self.poisoned {
            return Err(MySqlXaError::Poisoned);
        }
        self.poisoned = true;
        let mut connection = self.connection.take().ok_or(MySqlXaError::Poisoned)?;
        if sql.table_name().eq_ignore_ascii_case("undo_log")
            || sql
                .table_name()
                .to_ascii_lowercase()
                .starts_with("seata_rs_")
        {
            return Err(MySqlXaError::NotSent {
                operation: XaOperation::Execute,
                source: DbError::Input {
                    reason: "XA internal table is reserved",
                },
            });
        }
        MySqlImageReader::validate_parameters(sql, parameters).map_err(|source| {
            MySqlXaError::NotSent {
                operation: XaOperation::Execute,
                source,
            }
        })?;
        let arguments =
            MySqlImageReader::arguments(parameters).map_err(|source| MySqlXaError::NotSent {
                operation: XaOperation::Execute,
                source,
            })?;
        let deadline = Instant::now() + self.source.source.operation_timeout();
        let result = timeout_at(deadline, async {
            MySqlXaTable::verify(&mut connection, &self.source.source, sql.table_name())
                .await
                .map_err(|source| MySqlXaError::NotSent {
                    operation: XaOperation::Execute,
                    source,
                })?;
            sqlx::query_with(sql.sql(), arguments)
                .execute(&mut *connection)
                .await
                .map(|r| r.rows_affected())
                .map_err(|e| MySqlXaError::Unknown {
                    operation: XaOperation::Execute,
                    source: DbError::driver("XA business statement", e),
                })
        })
        .await
        .map_err(|_| MySqlXaError::Unknown {
            operation: XaOperation::Execute,
            source: DbError::Deadline {
                operation: "XA execute",
            },
        })??;
        if Instant::now() >= deadline {
            return Err(MySqlXaError::Unknown {
                operation: XaOperation::Execute,
                source: DbError::Deadline {
                    operation: "XA execute",
                },
            });
        }
        self.connection = Some(connection);
        self.poisoned = false;
        Ok(result)
    }
    /// 参数为原登记身份；仅受管理 Prepare 可在原 ACTIVE 连接写入提交见证，失败/取消即中毒。
    pub(crate) async fn insert_commit_witness(
        &mut self,
        identity: &seata_rm::BranchIdentity,
    ) -> Result<(), MySqlXaError> {
        if self.poisoned
            || identity.xid().as_bytes() != self.xid.get_global_transaction_id()
            || identity.branch_id() != self.xid.get_branch_id()
        {
            return Err(MySqlXaError::Poisoned);
        }
        self.poisoned = true;
        let mut connection = self.connection.take().ok_or(MySqlXaError::Poisoned)?;
        let deadline = Instant::now() + self.source.source.operation_timeout();
        timeout_at(
            deadline,
            crate::mysql_xa_commit_witness::MySqlXaCommitWitness::insert(
                &mut connection,
                &self.source.source,
                identity,
            ),
        )
        .await
        .map_err(|_| MySqlXaError::Unknown {
            operation: XaOperation::Execute,
            source: DbError::Deadline {
                operation: "XA commit witness",
            },
        })?
        .map_err(|source| MySqlXaError::Unknown {
            operation: XaOperation::Execute,
            source,
        })?;
        if Instant::now() >= deadline {
            return Err(MySqlXaError::Unknown {
                operation: XaOperation::Execute,
                source: DbError::Deadline {
                    operation: "XA commit witness",
                },
            });
        }
        self.connection = Some(connection);
        self.poisoned = false;
        Ok(())
    }
    /// 参数为原登记身份；对外暴露作用域前在原 ACTIVE 连接写入唯一 guard，失败/取消即中毒。
    pub(crate) async fn insert_session_guard(
        &mut self,
        identity: &seata_rm::BranchIdentity,
    ) -> Result<(), MySqlXaError> {
        if self.poisoned
            || identity.xid().as_bytes() != self.xid.get_global_transaction_id()
            || identity.branch_id() != self.xid.get_branch_id()
        {
            return Err(MySqlXaError::Poisoned);
        }
        self.poisoned = true;
        let mut connection = self.connection.take().ok_or(MySqlXaError::Poisoned)?;
        let deadline = Instant::now() + self.source.source.operation_timeout();
        timeout_at(
            deadline,
            crate::mysql_xa_session_fence::MySqlXaSessionFence::insert_guard(
                &mut connection,
                &self.source.source,
                identity,
            ),
        )
        .await
        .map_err(|_| MySqlXaError::Unknown {
            operation: XaOperation::Execute,
            source: DbError::Deadline {
                operation: "XA active guard",
            },
        })?
        .map_err(|source| MySqlXaError::Unknown {
            operation: XaOperation::Execute,
            source,
        })?;
        if Instant::now() >= deadline {
            return Err(MySqlXaError::Unknown {
                operation: XaOperation::Execute,
                source: DbError::Deadline {
                    operation: "XA active guard",
                },
            });
        }
        self.connection = Some(connection);
        self.poisoned = false;
        Ok(())
    }
    /// 消耗原 ACTIVE 作用域；返回原 END/PREPARE 明确确认的身份，Prepared 在原连接关闭后继续由 MySQL 持有。
    /// 失败/取消不能据此认定回滚；返回值不是 TC 登记回执，必须显式观察/解决同一身份。
    pub async fn prepare(mut self) -> Result<XaBranchXid, MySqlXaError> {
        if self.poisoned {
            return Err(MySqlXaError::Poisoned);
        }
        let mut connection = self.connection.take().ok_or(MySqlXaError::Poisoned)?;
        let args = self.xid.mysql_arguments()?;
        let deadline = Instant::now() + self.source.source.operation_timeout();
        MySqlXaDataSource::command(
            &mut connection,
            &format!("XA END {args}"),
            deadline,
            XaOperation::Prepare,
        )
        .await?;
        MySqlXaDataSource::command(
            &mut connection,
            &format!("XA PREPARE {args}"),
            deadline,
            XaOperation::Prepare,
        )
        .await?;
        Ok(self.xid)
    }
    /// 消耗仍活动的原作用域；在同连接单次 END/ROLLBACK，确认只代表本地 XA，不表示 TC 全局结果。
    pub async fn rollback(mut self) -> Result<(), MySqlXaError> {
        if self.poisoned {
            return Err(MySqlXaError::Poisoned);
        }
        let mut connection = self.connection.take().ok_or(MySqlXaError::Poisoned)?;
        let args = self.xid.mysql_arguments()?;
        let deadline = Instant::now() + self.source.source.operation_timeout();
        MySqlXaDataSource::command(
            &mut connection,
            &format!("XA END {args}"),
            deadline,
            XaOperation::Rollback,
        )
        .await?;
        MySqlXaDataSource::command(
            &mut connection,
            &format!("XA ROLLBACK {args}"),
            deadline,
            XaOperation::Rollback,
        )
        .await
    }
}
impl std::fmt::Debug for MySqlXaTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MySqlXaTransaction")
            .field("xid", &self.xid)
            .field("poisoned", &self.poisoned)
            .field("has_connection", &self.connection.is_some())
            .finish_non_exhaustive()
    }
}
