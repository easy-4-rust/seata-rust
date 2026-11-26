use crate::mysql_at_registration_checks::MySqlAtRegistrationChecks;
use crate::mysql_image_reader::MySqlImageReader;
use crate::mysql_resource_binding::MySqlResourceBinding;
use crate::{cache::MySqlTableMetaReader, DbError};
use crate::{
    exec::{DeleteExecutor, UpdateExecutor},
    mysql::MySqlInsertExecutor,
};
use seata_at::{SqlRecognizer, SqlType, SqlUndoLog};
use seata_datasource::{FieldValue, ImageError, TableMeta, TableRecords};
use sqlx::{MySql, Transaction};
use std::{fmt, time::Duration};
use tokio::time::{timeout_at, Instant};

/// 单一连接的本地事务作用域；失败或取消后只允许回滚/Drop，当前不暴露绕过Undo的提交入口。
/// 对应 Java: org.apache.seata.rm.datasource.ConnectionProxy的本地事务/元数据基础路径（部分实现）。
pub struct MySqlLocalTransaction {
    transaction: Option<Transaction<'static, MySql>>,
    database: String,
    resource_id: Option<String>,
    server_uuid: std::sync::Arc<str>,
    metadata_options: std::sync::Arc<sqlx::mysql::MySqlConnectOptions>,
    registration_checks: MySqlAtRegistrationChecks,
    poisoned: bool,
    operation_timeout: Duration,
    undo_logs: Vec<SqlUndoLog>,
    undo_bytes: usize,
    undo_flushed: bool,
    phase_one_metrics: crate::MySqlPhaseOneMetricsSampler,
}
impl MySqlLocalTransaction {
    /// 参数为原成功回执和固定配置；在原物理事务中保持恢复行及 Normal 锁，期限后不发布成功。
    pub(crate) async fn lock_commit_preconditions(
        &mut self,
        receipt: Option<&seata_rm::RegisteredBranch>,
        profile: seata_at::MySqlJdbcProfile,
    ) -> Result<bool, DbError> {
        if self.poisoned {
            return Err(DbError::Poisoned);
        }
        if self.transaction.is_none() {
            return Err(DbError::Closed);
        }
        if self.undo_logs.is_empty() {
            return if receipt.is_none() {
                Ok(false)
            } else {
                Err(DbError::RecoveryConflict)
            };
        }
        if !self.undo_flushed {
            return Err(DbError::RecoveryConflict);
        }
        let receipt = receipt.ok_or(DbError::RecoveryConflict)?;
        let id = receipt.identity();
        if self.resource_id.as_deref() != Some(id.resource().resource_id()) {
            return Err(DbError::RecoveryConflict);
        }
        self.poisoned = true;
        // 首次 await 前移走实际事务；取消或任何失败都不能留下仍可使用的半检查连接。
        let mut transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(deadline, async {
            let schema = crate::rm_branch_store::RmBranchStore::validate_table(
                &mut transaction,
                &self.metadata_options,
                &self.database,
            )
            .await?;
            crate::rm_branch_store::RmBranchStore::check_commit(
                &mut transaction,
                receipt,
                profile,
                schema,
            )
            .await?;
            crate::mysql::MySqlUndoLogManager::validate_table(&mut transaction, &self.database)
                .await?;
            let (actual, _) = crate::mysql::MySqlUndoLogManager::read_normal(
                &mut transaction,
                id.xid(),
                id.branch_id(),
            )
            .await?;
            let expected = seata_at::JacksonUndoLogParser::default().encode_prepared(
                id.xid(),
                id.branch_id(),
                &self.undo_logs,
                profile,
            )?;
            if actual != expected {
                return Err(DbError::RecoveryConflict);
            }
            crate::rm_branch_store::RmBranchStore::stage_first_phase_witness(
                &mut transaction,
                id,
                schema,
            )
            .await?;
            Ok(schema.has_witness())
        })
        .await;
        if Instant::now() >= deadline {
            return Err(DbError::Deadline {
                operation: "commit preconditions",
            });
        }
        let witness_staged = result.map_err(|_| DbError::Deadline {
            operation: "commit preconditions",
        })??;
        self.transaction = Some(transaction);
        self.poisoned = false;
        Ok(witness_staged)
    }
    /// 参数为目标资源、实例身份和 JDBC 配置；返回实际镜像的原锁集合，失败时不申请分支。
    pub(crate) fn preparation_lock_keys(
        &self,
        resource: &str,
        server_uuid: &str,
        profile: seata_at::MySqlJdbcProfile,
    ) -> Result<Option<String>, crate::MySqlPrepareError> {
        let source = if self.poisoned {
            Some(DbError::Poisoned)
        } else if self.transaction.is_none() {
            Some(DbError::Closed)
        } else if self.undo_flushed {
            Some(DbError::UndoSealed)
        } else if self.resource_id.as_deref() != Some(resource) {
            Some(DbError::Input {
                reason: "preparation resource differs from original transaction",
            })
        } else if !self.undo_logs.is_empty() && self.server_uuid.as_ref() != server_uuid {
            Some(DbError::Unsupported {
                feature: "preparation requires the original physical MySQL schema",
            })
        } else {
            None
        };
        if let Some(source) = source {
            return Err(crate::MySqlPrepareError::Local { source });
        }
        Ok(
            seata_at::ConnectionContext::for_phase_one_registration(&self.undo_logs, profile)?
                .build_lock_keys(),
        )
    }
    /// 参数为原 JDBC 标识、实际锁键、JDBC 配置和管理器要求；任一池启用的校验均在登记前执行。
    /// 排序规则和资源绑定在持有业务表元数据锁的原事务上核对。
    pub(crate) async fn verify_registration_checks(
        &mut self,
        resource_id: &str,
        lock_key: Option<&str>,
        profile: seata_at::MySqlJdbcProfile,
        manager_checks: MySqlAtRegistrationChecks,
    ) -> Result<(), crate::MySqlPrepareError> {
        let transaction_checks = self.registration_checks;
        if let Some(lock_key) = lock_key.filter(|_| {
            transaction_checks.java_tc_mysql_lock_limits || manager_checks.java_tc_mysql_lock_limits
        }) {
            crate::java_tc_mysql_lock_limits::JavaTcMysqlLockLimits::verify(resource_id, lock_key)?;
        }
        let transaction = self
            .transaction
            .as_mut()
            .ok_or(crate::MySqlPrepareError::Local {
                source: DbError::Closed,
            })?;
        if transaction_checks.java_tc_mysql_lock_collation
            || manager_checks.java_tc_mysql_lock_collation
        {
            crate::java_tc_mysql_lock_collation::JavaTcMysqlLockCollation::verify(
                transaction,
                &self.undo_logs,
            )
            .await
            .map_err(|source| crate::MySqlPrepareError::Local { source })?;
        }
        if transaction_checks.require_resource_binding || manager_checks.require_resource_binding {
            let check_profile = transaction_checks.require_tinyint_profile_binding
                || manager_checks.require_tinyint_profile_binding;
            MySqlResourceBinding::lock_verified(
                transaction,
                resource_id,
                check_profile.then_some(profile),
            )
            .await
            .map_err(|source| crate::MySqlPrepareError::Local { source })?;
        }
        Ok(())
    }
    /// 参数为原分支身份与明确JDBC配置；同事务写入原Undo行并封存作用域，不提交或证明TC登记。
    /// 对应 Java: AbstractUndoLogManager#flushUndoLogs；失败/取消后只允许整体回滚或丢弃。
    pub async fn flush_undo_logs(
        &mut self,
        xid: &str,
        branch_id: i64,
        profile: seata_at::MySqlJdbcProfile,
    ) -> Result<(), DbError> {
        if self.poisoned {
            return Err(DbError::Poisoned);
        }
        if self.undo_flushed {
            return Err(DbError::UndoSealed);
        }
        if self.undo_logs.is_empty() {
            self.undo_flushed = true;
            return Ok(());
        }
        self.poisoned = true;
        // 编码/SQL共用同一期限；首次await前移走实际事务，取消不会留下可复用的旧连接。
        let mut transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(deadline, async {
            let bytes = seata_at::JacksonUndoLogParser::default().encode_prepared(
                xid,
                branch_id,
                &self.undo_logs,
                profile,
            )?;
            if Instant::now() >= deadline {
                return Err(DbError::Deadline {
                    operation: "Undo flush",
                });
            }
            crate::mysql::MySqlUndoLogManager::flush(
                &mut transaction,
                &self.metadata_options,
                &self.database,
                xid,
                branch_id,
                &bytes,
            )
            .await
        })
        .await;
        match result {
            Ok(result) => self.complete_undo_flush(transaction, result, deadline),
            Err(_) => {
                drop(transaction);
                Err(DbError::Deadline {
                    operation: "Undo flush",
                })
            }
        }
    }
    fn complete_undo_flush(
        &mut self,
        transaction: Transaction<'static, MySql>,
        result: Result<(), DbError>,
        deadline: Instant,
    ) -> Result<(), DbError> {
        // Tokio Timeout先poll内部Future；Ready也必须再次检查，不能在期限后发布成功。
        if Instant::now() >= deadline {
            drop(transaction);
            return Err(DbError::Deadline {
                operation: "Undo flush",
            });
        }
        self.transaction = Some(transaction);
        result?;
        self.undo_flushed = true;
        self.poisoned = false;
        Ok(())
    }
    /// 参数为正式SQL及全部原绑定；只执行一次并准备内存Undo，返回原更新计数；本地事务不公开提交入口。
    /// 对应Java: org.apache.seata.rm.datasource.ConnectionProxy的语句/Undo准备事务边界（部分实现）。
    pub async fn execute(
        &mut self,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<u64, DbError> {
        let guard = self
            .phase_one_metrics
            .begin(crate::MySqlPhaseOneOperation::Execute);
        let result = self.execute_inner(sql, parameters).await;
        guard.finish_execute(&result);
        result
    }
    async fn execute_inner(
        &mut self,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<u64, DbError> {
        if self.poisoned {
            return Err(DbError::Poisoned);
        }
        if self.undo_flushed {
            return Err(DbError::UndoSealed);
        }
        if sql
            .table_name()
            .eq_ignore_ascii_case(MySqlResourceBinding::TABLE)
        {
            return Err(DbError::Input {
                reason: "AT resource binding is reserved for administrative provisioning",
            });
        }
        MySqlImageReader::validate_parameters(sql, parameters)?;
        if self.undo_logs.len() >= 128 {
            self.poisoned = true;
            return Err(ImageError::Budget {
                limit: "128 pending Undo records",
            }
            .into());
        }
        self.poisoned = true;
        // 首次await前把实际事务移入Future，取消也不会留下可复用的半完成业务作用域。
        let mut transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(deadline, async {
            crate::current_table_constraints::CurrentTableConstraints::verify(
                &mut transaction,
                &self.metadata_options,
                &self.database,
                sql.table_name(),
            )
            .await?;
            match sql.sql_type() {
                SqlType::Insert => {
                    MySqlInsertExecutor::execute(&mut transaction, &self.database, sql, parameters)
                        .await
                }
                SqlType::Update => {
                    UpdateExecutor::execute(&mut transaction, &self.database, sql, parameters).await
                }
                SqlType::Delete => {
                    DeleteExecutor::execute(&mut transaction, &self.database, sql, parameters).await
                }
            }
        })
        .await;
        match result {
            Ok(result) => {
                self.transaction = Some(transaction);
                let (count, undo) = result?;
                Self::verify_affected_rows_have_image(count, &undo)?;
                // 本次DML可能已生效但未提交；准备日志预算失败必须保留poison并整体回滚。
                if !(undo.before_image().rows().is_empty() && undo.after_image().rows().is_empty())
                {
                    let bytes = self.undo_bytes.checked_add(undo.value_bytes()).ok_or(
                        ImageError::Budget {
                            limit: "pending Undo bytes overflow",
                        },
                    )?;
                    if bytes > 16_777_216 {
                        return Err(ImageError::Budget {
                            limit: "16MiB pending Undo payload",
                        }
                        .into());
                    }
                    self.undo_logs.push(undo);
                    self.undo_bytes = bytes;
                }
                self.poisoned = false;
                Ok(count)
            }
            Err(_) => {
                drop(transaction);
                Err(DbError::Deadline {
                    operation: "business and images",
                })
            }
        }
    }
    /// 有实际影响的 DML 若没有前后镜像，整笔作用域保持中毒，禁止误走无变化提交。
    fn verify_affected_rows_have_image(count: u64, undo: &SqlUndoLog) -> Result<(), DbError> {
        if count > 0
            && undo.before_image().rows().is_empty()
            && undo.after_image().rows().is_empty()
        {
            return Err(ImageError::Invalid {
                reason: "affected business rows require Undo image",
            }
            .into());
        }
        Ok(())
    }
    /// 无参数；只读查看事务内准备数据，不表示Undo已落库或业务已提交。
    pub fn undo_logs(&self) -> &[SqlUndoLog] {
        &self.undo_logs
    }
    /// 参数为正式UPDATE/DELETE识别结果与完整原绑定；返回同事务持锁完整前镜像，不执行业务SQL。
    /// 对应Java: org.apache.seata.rm.datasource.exec.UpdateExecutor#beforeImage及DeleteExecutor；部分实现。
    pub async fn before_image(
        &mut self,
        sql: &SqlRecognizer,
        parameters: &[FieldValue],
    ) -> Result<TableRecords, DbError> {
        if self.poisoned {
            return Err(DbError::Poisoned);
        }
        if self.undo_flushed {
            return Err(DbError::UndoSealed);
        }
        MySqlImageReader::validate_request(sql, parameters)?;
        self.poisoned = true;
        let mut transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(deadline, async {
            crate::current_table_constraints::CurrentTableConstraints::verify(
                &mut transaction,
                &self.metadata_options,
                &self.database,
                sql.table_name(),
            )
            .await?;
            MySqlImageReader::read(&mut transaction, &self.database, sql, parameters).await
        })
        .await;
        match result {
            Ok(result) => {
                self.transaction = Some(transaction);
                if result.is_ok() {
                    self.poisoned = false;
                }
                result
            }
            Err(_) => {
                drop(transaction);
                Err(DbError::Deadline {
                    operation: "before image",
                })
            }
        }
    }
    /// 参数为独占SQLx事务及固定schema；返回可取消但不可共享连接的作用域。
    #[allow(clippy::too_many_arguments)] // 同一数据源的原连接配置与只读计数归属必须一起交接。
    pub(crate) fn new(
        transaction: Transaction<'static, MySql>,
        database: String,
        resource_id: Option<String>,
        server_uuid: std::sync::Arc<str>,
        registration_checks: MySqlAtRegistrationChecks,
        metadata_options: std::sync::Arc<sqlx::mysql::MySqlConnectOptions>,
        operation_timeout: Duration,
        phase_one_metrics: crate::MySqlPhaseOneMetricsSampler,
    ) -> Self {
        Self {
            transaction: Some(transaction),
            database,
            resource_id,
            server_uuid,
            metadata_options,
            registration_checks,
            poisoned: false,
            operation_timeout,
            undo_logs: Vec::new(),
            undo_bytes: 0,
            undo_flushed: false,
            phase_one_metrics,
        }
    }
    /// 参数为原单表名；在同事务持有MDL后读取完整元数据，错误或取消保持poison。
    pub async fn table_meta(&mut self, table: &str) -> Result<TableMeta, DbError> {
        if self.poisoned {
            return Err(DbError::Poisoned);
        }
        if self.undo_flushed {
            return Err(DbError::UndoSealed);
        }
        // 在首次await之前立起失败状态，Future被取消也不能继续复用半完成作用域。
        self.poisoned = true;
        // Future暂时拥有整个事务；调用被取消时事务立即Drop，而不是遗留在外层对象中。
        let mut transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(deadline, async {
            crate::current_table_constraints::CurrentTableConstraints::verify(
                &mut transaction,
                &self.metadata_options,
                &self.database,
                table,
            )
            .await?;
            MySqlTableMetaReader::fetch(&mut transaction, &self.database, table).await
        })
        .await;
        match result {
            Ok(result) => {
                self.transaction = Some(transaction);
                if result.is_ok() {
                    self.poisoned = false;
                }
                result
            }
            Err(_) => {
                drop(transaction);
                Err(DbError::Deadline {
                    operation: "table metadata",
                })
            }
        }
    }
    /// 无参数；显式回滚并归还连接，返回原驱动分类；Drop仍由SQLx负责安全回滚。
    pub async fn rollback(self) -> Result<(), DbError> {
        let guard = self
            .phase_one_metrics
            .begin(crate::MySqlPhaseOneOperation::Rollback);
        let result = self.rollback_inner().await;
        guard.finish_rollback(&result);
        result
    }
    async fn rollback_inner(mut self) -> Result<(), DbError> {
        let transaction = self.transaction.take().ok_or(DbError::Closed)?;
        timeout_at(
            Instant::now() + self.operation_timeout,
            transaction.rollback(),
        )
        .await
        .map_err(|_| DbError::Deadline {
            operation: "rollback",
        })?
        .map_err(|error| DbError::driver("rollback", error))
    }
    /// 消耗已确认无变化的原本地事务，在原操作期限内发送一次真实 COMMIT。
    /// 驱动失败、期限或期限后完成均由上层保守归为结果未知。
    /// 对应 Java: ConnectionProxy#processGlobalTransactionCommit 的 targetConnection.commit。
    pub(crate) async fn commit_no_change(self) -> Result<(), DbError> {
        self.commit_local("no-change COMMIT").await
    }

    #[cfg(feature = "experimental_changed_commit")]
    /// 消耗已仲裁且暂存 004 见证的原事务，一次提交业务、Normal Undo 和候选见证。
    /// 此内部能力不证明 TC 锁安全或独立持久事实，由上层实验入口继续观察。
    pub(crate) async fn commit_changed(self) -> Result<(), DbError> {
        self.commit_local("changed COMMIT").await
    }

    async fn commit_local(mut self, operation: &'static str) -> Result<(), DbError> {
        let transaction = self.transaction.take().ok_or(DbError::Closed)?;
        let deadline = Instant::now() + self.operation_timeout;
        let result = timeout_at(deadline, transaction.commit()).await;
        if Instant::now() >= deadline {
            return Err(DbError::Deadline { operation });
        }
        result
            .map_err(|_| DbError::Deadline { operation })?
            .map_err(|error| DbError::driver(operation, error))
    }
}

impl fmt::Debug for MySqlLocalTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlLocalTransaction")
            .field("poisoned", &self.poisoned)
            .field("open", &self.transaction.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::MySqlLocalTransaction;
    use crate::{DbError, MySqlDataSource, MySqlOptions};
    use seata_at::{
        JacksonUndoLogParser, MySqlJdbcProfile, SqlRecognizerFactory, SqlType, SqlUndoLog,
    };
    use seata_datasource::{ColumnMeta, ColumnType, ImageError, TableMeta, TableRecords};
    use sqlx::Connection;
    use std::time::Duration;
    use tokio::time::Instant;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn affected_business_rows_without_any_undo_image_cannot_be_no_change() {
        let id = ColumnMeta::new(
            "id".into(),
            ColumnType::SignedInteger,
            false,
            1,
            Some(1),
            None,
            None,
            None,
        )
        .unwrap();
        let meta = TableMeta::new("undo_work".into(), vec![id]).unwrap();
        let empty = TableRecords::new(meta);
        let undo = SqlUndoLog::new(SqlType::Update, empty.clone(), empty).unwrap();
        assert!(matches!(
            MySqlLocalTransaction::verify_affected_rows_have_image(1, &undo),
            Err(DbError::Image(ImageError::Invalid {
                reason: "affected business rows require Undo image"
            }))
        ));
        assert!(MySqlLocalTransaction::verify_affected_rows_have_image(0, &undo).is_ok());
    }

    #[tokio::test]
    #[ignore = "requires owned real MySQL fixture"]
    #[allow(clippy::unwrap_used)]
    async fn ready_undo_publication_after_deadline_keeps_poison_and_rolls_back_original_transaction(
    ) {
        let options = MySqlOptions::new(
            "127.0.0.1".into(),
            std::env::var("SEATA_MYSQL_PORT").unwrap().parse().unwrap(),
            "root".into(),
            std::env::var("SEATA_MYSQL_PASSWORD").unwrap(),
            std::env::var("SEATA_MYSQL_DATABASE").unwrap(),
        )
        .unwrap()
        .with_insecure_loopback_tls()
        .unwrap();
        let mut observer = sqlx::MySqlConnection::connect_with(&options.connect_options())
            .await
            .unwrap();
        sqlx::query("CREATE TABLE undo_ready(id INT PRIMARY KEY) ENGINE=InnoDB")
            .execute(&mut observer)
            .await
            .unwrap();
        let ds = MySqlDataSource::connect(options).await.unwrap();
        let mut scope: MySqlLocalTransaction = ds.begin().await.unwrap();
        scope
            .execute(
                &SqlRecognizerFactory::recognize("INSERT INTO undo_ready(id) VALUES(1)").unwrap(),
                &[],
            )
            .await
            .unwrap();
        scope.poisoned = true;
        let mut transaction = scope.transaction.take().unwrap();
        let bytes = JacksonUndoLogParser::default()
            .encode_prepared(
                "ready:xid",
                42,
                &scope.undo_logs,
                MySqlJdbcProfile::Connector8Default,
            )
            .unwrap();
        let ready = crate::mysql::MySqlUndoLogManager::flush(
            &mut transaction,
            &scope.metadata_options,
            &scope.database,
            "ready:xid",
            42,
            &bytes,
        )
        .await;
        assert!(ready.is_ok());
        // 用实际完成的最后写入，显式延后其状态发布；覆盖Ready交接处，不伪称网络丢包试验。
        let deadline = Instant::now() + Duration::from_millis(10);
        tokio::time::sleep(Duration::from_millis(20)).await;
        let result = scope.complete_undo_flush(transaction, ready, deadline);
        assert!(
            matches!(
                result,
                Err(DbError::Deadline {
                    operation: "Undo flush"
                })
            ),
            "expired ready result must not publish successful Undo"
        );
        assert!(scope.poisoned && scope.transaction.is_none() && !scope.undo_flushed);
        ds.close().await.unwrap();
        for _ in 0..100 {
            let rows: i64 = sqlx::query_scalar(
                "SELECT (SELECT COUNT(*) FROM undo_ready)+(SELECT COUNT(*) FROM undo_log)",
            )
            .fetch_one(&mut observer)
            .await
            .unwrap();
            if rows == 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("expired original transaction was not rolled back");
    }
}
