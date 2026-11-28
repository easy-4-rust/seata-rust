use crate::{
    cache::MySqlTableMetaReader, current_table_constraints::CurrentTableConstraints, DbError,
    MySqlDataSource,
};
use seata_datasource::TableMeta;
use sqlx::{Connection, MySqlConnection, Row};
/// 持原 XA 元数据锁时检查当前物理引擎与同实例新快照约束。
/// 对应 Java: 无直接对象；Rust XA 有界 DML 的跨快照准入。
pub(crate) struct MySqlXaTable;
impl MySqlXaTable {
    /// 参数为独占 XA 连接、原数据源与目标表；全部检查在调用方绝对期限内，无成功 DML 副作用。
    pub(crate) async fn verify(
        connection: &mut MySqlConnection,
        source: &MySqlDataSource,
        table: &str,
    ) -> Result<(), DbError> {
        let quoted = TableMeta::quote_identifier(table)?;
        sqlx::query(&format!("SELECT 1 FROM {quoted} LIMIT 0 FOR UPDATE"))
            .execute(&mut *connection)
            .await
            .map_err(|e| DbError::driver("XA metadata lock", e))?;
        let row = sqlx::query(&format!("SHOW CREATE TABLE {quoted}"))
            .fetch_one(&mut *connection)
            .await
            .map_err(|e| DbError::driver("XA current engine", e))?;
        let bytes: &[u8] = row
            .try_get(1)
            .map_err(|e| DbError::driver("XA engine decode", e))?;
        let definition = std::str::from_utf8(bytes).map_err(|_| DbError::Unsupported {
            feature: "XA table definition encoding",
        })?;
        MySqlTableMetaReader::verify_engine_definition(definition)?;
        let mut current = MySqlConnection::connect_with(&source.control_pool().connect_options())
            .await
            .map_err(|e| DbError::driver("XA current metadata connect", e))?;
        let identity: (String, String) = sqlx::query_as("SELECT @@server_uuid,DATABASE()")
            .fetch_one(&mut current)
            .await
            .map_err(|e| DbError::driver("XA current metadata identity", e))?;
        if identity.0 != source.server_uuid.as_ref() || identity.1 != source.database() {
            return Err(DbError::RecoveryConflict);
        }
        let (trigger, foreign_key) =
            CurrentTableConstraints::read(&mut current, source.database(), table).await?;
        current
            .close()
            .await
            .map_err(|e| DbError::driver("XA current metadata close", e))?;
        if trigger || foreign_key {
            return Err(DbError::Unsupported {
                feature: "XA trigger or foreign key",
            });
        }
        Ok(())
    }
}
