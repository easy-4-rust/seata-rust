use super::UndoSerializer;
use crate::DbError;
use seata_at::{
    Fastjson2UndoDocument, Fastjson2UndoLogParser, FastjsonUndoDocument, FastjsonUndoLogParser,
    JacksonUndoLogParser, MySqlJdbcProfile, SqlUndoLog, UndoLogDocument,
};
use seata_datasource::TableMeta;
use std::collections::HashSet;

/// 保留锁定格式对应的只读文档，确保查询元数据与绑定使用同一份载荷。
/// 对应 Java: AbstractUndoLogManager#undo 的解析结果。
pub(crate) enum RollbackUndoDocument {
    Jackson(UndoLogDocument),
    Fastjson(FastjsonUndoDocument),
    Fastjson2(Fastjson2UndoDocument),
}

impl RollbackUndoDocument {
    pub(crate) fn decode(
        format: UndoSerializer,
        bytes: &[u8],
        xid: &str,
        branch_id: i64,
    ) -> Result<Self, DbError> {
        match format {
            UndoSerializer::Jackson => {
                let document =
                    JacksonUndoLogParser::default()
                        .decode(bytes)?
                        .ok_or(DbError::Input {
                            reason: "Undo non-null original document",
                        })?;
                if document.xid() != Some(xid) || document.branch_id() != branch_id {
                    return Err(DbError::Input {
                        reason: "Undo original document identity",
                    });
                }
                Ok(Self::Jackson(document))
            }
            UndoSerializer::Fastjson => Ok(Self::Fastjson(
                FastjsonUndoLogParser::default().decode_for_restore(bytes, xid, branch_id)?,
            )),
            UndoSerializer::Fastjson2 => Ok(Self::Fastjson2(
                Fastjson2UndoLogParser::default().decode_for_restore(bytes, xid, branch_id)?,
            )),
        }
    }

    pub(crate) fn table_names(&self) -> Result<Vec<&str>, DbError> {
        let document = match self {
            Self::Jackson(document) => document,
            Self::Fastjson(document) => return Ok(document.table_names().collect()),
            Self::Fastjson2(document) => return Ok(document.table_names().collect()),
        };
        let logs = document.sql_undo_logs().ok_or(DbError::Input {
            reason: "Undo concrete SQL logs",
        })?;
        let mut seen = HashSet::new();
        let mut tables = Vec::new();
        for log in logs {
            let name = log
                .get("tableName")
                .and_then(|value| value.as_str())
                .ok_or(DbError::Input {
                    reason: "Undo concrete table name",
                })?;
            if seen.insert(name) {
                tables.push(name);
            }
        }
        Ok(tables)
    }

    pub(crate) fn bind(
        &self,
        xid: &str,
        branch_id: i64,
        tables: &[TableMeta],
        profile: MySqlJdbcProfile,
    ) -> Result<Vec<SqlUndoLog>, DbError> {
        Ok(match self {
            Self::Jackson(document) => JacksonUndoLogParser::default()
                .restore_prepared(document, xid, branch_id, tables, profile)?,
            Self::Fastjson(document) => FastjsonUndoLogParser::default()
                .restore_document(document, xid, branch_id, tables, profile)?,
            Self::Fastjson2(document) => document.bind(xid, branch_id, tables, profile)?,
        })
    }
}
