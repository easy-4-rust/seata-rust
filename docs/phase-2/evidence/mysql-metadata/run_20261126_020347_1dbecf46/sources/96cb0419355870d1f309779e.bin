use crate::data_compare_utils::DataCompareUtils;
use crate::{
    cache::MySqlTableMetaReader,
    mysql::{MySqlUndoDeleteExecutor, MySqlUndoInsertExecutor, MySqlUndoUpdateExecutor},
    mysql_image_reader::MySqlImageReader,
    DbError,
};
use seata_at::{SqlType, SqlUndoLog};

use sqlx::{MySql, Transaction};

/// 在原独占事务中校验当前镜像并补偿一条日志；不提交、不重发原业务SQL。
/// 对应 Java: org.apache.seata.rm.datasource.undo.AbstractUndoExecutor（已有完整镜像类型范围）。
pub(crate) struct AbstractUndoExecutor;
impl AbstractUndoExecutor {
    /// 参数为原事务/schema/日志；返回实际恢复行数，脏数据和任何失败由调用者整体回滚。
    /// 对应 Java: AbstractUndoExecutor#executeOn/dataValidationAndGoOn/queryCurrentRecords。
    pub(crate) async fn execute(
        tx: &mut Transaction<'static, MySql>,
        database: &str,
        log: &SqlUndoLog,
    ) -> Result<u64, DbError> {
        let table = MySqlTableMetaReader::fetch(tx, database, log.table_name()).await?;
        if &table != log.before_image().table_meta() {
            return Err(DbError::Unsupported {
                feature: "Undo physical schema changed",
            });
        }
        if DataCompareUtils::records_equal(log.before_image(), log.after_image()) {
            return Ok(0);
        }
        let undo_rows = if log.sql_type() == SqlType::Insert {
            log.after_image()
        } else {
            log.before_image()
        };
        let keys = MySqlImageReader::primary_values(undo_rows)?;
        let current = MySqlImageReader::read_keys(tx, &table, &keys).await?;
        if !DataCompareUtils::records_equal(&current, log.after_image()) {
            if DataCompareUtils::records_equal(&current, log.before_image()) {
                return Ok(0);
            }
            return Err(DbError::UndoDirty);
        }
        let mut count = 0;
        for row in undo_rows.rows() {
            let changed = match log.sql_type() {
                SqlType::Insert => MySqlUndoInsertExecutor::execute(tx, &table, row).await?,
                SqlType::Update => MySqlUndoUpdateExecutor::execute(tx, &table, row).await?,
                SqlType::Delete => MySqlUndoDeleteExecutor::execute(tx, &table, row).await?,
            };
            if changed != 1 {
                return Err(DbError::Input {
                    reason: "Undo exact affected row count",
                });
            }
            count += changed;
        }
        if !DataCompareUtils::records_equal(
            &MySqlImageReader::read_keys(tx, &table, &keys).await?,
            log.before_image(),
        ) {
            return Err(DbError::UndoDirty);
        }
        Ok(count)
    }
}
