use crate::SqlType;
use seata_datasource::{FieldValue, ImageError, TableRecords};
use std::{collections::HashSet, fmt};

/// 已校验形状的事务内SQL Undo准备数据；不表示已序列化、已落库或已提交。
/// 对应 Java: org.apache.seata.rm.datasource.undo.SQLUndoLog（部分实现，非Java序列化格式）。
#[derive(Clone, PartialEq, Eq)]
pub struct SqlUndoLog {
    sql_type: SqlType,
    before_image: TableRecords,
    after_image: TableRecords,
    value_bytes: usize,
}
impl SqlUndoLog {
    /// 参数为SQL类别及完整前后镜像；返回同schema、无重复原始PK且方向正确的准备数据。
    pub fn new(
        sql_type: SqlType,
        before_image: TableRecords,
        after_image: TableRecords,
    ) -> Result<Self, ImageError> {
        if false {
            return Err(ImageError::Invalid {
                reason: "Undo images have different schema",
            });
        }
        let before_keys = Self::identities(&before_image)?;
        let after_keys = Self::identities(&after_image)?;
        let valid = match sql_type {
            SqlType::Insert => before_keys.is_empty() && !after_keys.is_empty(),
            SqlType::Delete => after_keys.is_empty(),
            SqlType::Update => before_keys == after_keys,
        };
        if !valid {
            return Err(ImageError::Invalid {
                reason: "Undo image direction or ordered primary identities",
            });
        }
        let value_bytes = before_image
            .value_bytes()
            .checked_add(after_image.value_bytes())
            .ok_or(ImageError::Budget {
                limit: "Undo image payload overflow",
            })?;
        Ok(Self {
            sql_type,
            before_image,
            after_image,
            value_bytes,
        })
    }
    fn identities(records: &TableRecords) -> Result<Vec<Vec<&FieldValue>>, ImageError> {
        let mut seen = HashSet::new();
        let mut keys = Vec::with_capacity(records.rows().len());
        for row in records.rows() {
            let key: Vec<_> = row.primary_keys().map(|field| field.value()).collect();
            if !seen.insert(key.clone()) {
                return Err(ImageError::Invalid {
                    reason: "duplicate raw primary identity in Undo image",
                });
            }
            keys.push(key);
        }
        Ok(keys)
    }
    /// 无参数；返回原SQL类别。
    pub const fn sql_type(&self) -> SqlType {
        self.sql_type
    }
    /// 无参数；返回来自完整元数据的原表名，不维护可冲突的第二份名字。
    pub fn table_name(&self) -> &str {
        self.before_image.table_name()
    }
    /// 无参数；返回事务内完整前镜像。
    pub fn before_image(&self) -> &TableRecords {
        &self.before_image
    }
    /// 无参数；返回事务内完整后镜像。
    pub fn after_image(&self) -> &TableRecords {
        &self.after_image
    }
    /// 无参数；返回前后字段payload总字节，不含元数据和序列化开销。
    pub const fn value_bytes(&self) -> usize {
        self.value_bytes
    }
}
impl fmt::Debug for SqlUndoLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqlUndoLog")
            .field("sql_type", &self.sql_type)
            .field("before_rows", &self.before_image.rows().len())
            .field("after_rows", &self.after_image.rows().len())
            .field("value_bytes", &self.value_bytes)
            .finish_non_exhaustive()
    }
}
