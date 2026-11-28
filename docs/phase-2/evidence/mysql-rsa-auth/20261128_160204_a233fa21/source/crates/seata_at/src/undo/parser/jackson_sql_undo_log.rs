use super::JacksonUndoRecords;

/// Jackson 原 SQLUndoLog 对象，保留可空 SQLType、表名和镜像。
/// 对应 Java: org.apache.seata.rm.datasource.undo.SQLUndoLog。
#[derive(Clone, PartialEq)]
pub struct JacksonSqlUndoLog {
    pub(super) sql_type: Option<String>,
    pub(super) table: Option<String>,
    pub(super) before: Option<JacksonUndoRecords>,
    pub(super) after: Option<JacksonUndoRecords>,
}

impl JacksonSqlUndoLog {
    /// 参数为原 SQLType 标签、表名及前后镜像，返回普通日志；编码验证 SQLType 标签。
    pub fn new(
        sql_type: Option<String>,
        table: Option<String>,
        before: Option<JacksonUndoRecords>,
        after: Option<JacksonUndoRecords>,
    ) -> Self {
        Self {
            sql_type,
            table,
            before,
            after,
        }
    }

    /// 无参数；返回原 SQLType 标签。
    pub fn sql_type(&self) -> Option<&str> {
        self.sql_type.as_deref()
    }

    /// 无参数；返回原日志表名。
    pub fn table_name(&self) -> Option<&str> {
        self.table.as_deref()
    }

    /// 无参数；返回可空的前镜像。
    pub fn before_image(&self) -> Option<&JacksonUndoRecords> {
        self.before.as_ref()
    }

    /// 无参数；返回可空的后镜像。
    pub fn after_image(&self) -> Option<&JacksonUndoRecords> {
        self.after.as_ref()
    }
}
