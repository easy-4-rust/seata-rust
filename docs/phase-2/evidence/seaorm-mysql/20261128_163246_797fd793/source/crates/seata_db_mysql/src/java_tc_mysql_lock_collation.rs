use crate::DbError;
use seata_at::SqlUndoLog;
use seata_datasource::{ColumnType, TableMeta};
use sqlx::{MySql, Transaction};
use std::collections::BTreeSet;

/// 原 Java TC MySQL 锁表使用 utf8mb4_0900_ai_ci 时的业务主键排序规则校验。
/// 对应 Java: 无直接对象；校验原版 lock_table.row_key 的部署前提。
pub(crate) struct JavaTcMysqlLockCollation;

impl JavaTcMysqlLockCollation {
    /// 参数为持有业务表元数据锁的事务与其 Undo；缺失或不匹配的文本主键列会拒绝登记。
    pub(crate) async fn verify(
        transaction: &mut Transaction<'static, MySql>,
        logs: &[SqlUndoLog],
    ) -> Result<(), DbError> {
        let mut checked = BTreeSet::new();
        for log in logs {
            let meta = log.after_image().table_meta();
            for column in meta.primary_key_columns() {
                if column.kind() != ColumnType::Text
                    || !checked.insert((meta.name(), column.name()))
                {
                    continue;
                }
                // 从当前已解析的物理列类型取得排序规则，空表同样有类型。
                // 标识符由原元数据验证并引用，原事务继续持有业务表的 MDL。
                let table = TableMeta::quote_identifier(meta.name())?;
                let column = TableMeta::quote_identifier(column.name())?;
                let sql = format!("SELECT COLLATION((SELECT {column} FROM {table} LIMIT 0))");
                let collation: String = sqlx::query_scalar(&sql)
                    .fetch_one(&mut **transaction)
                    .await
                    .map_err(|error| {
                        DbError::driver("verify AT text primary key collation", error)
                    })?;
                if collation != "utf8mb4_0900_ai_ci" {
                    return Err(DbError::Unsupported {
                        feature: "business text primary key collation differs from Java TC MySQL lock storage",
                    });
                }
            }
        }
        Ok(())
    }
}
