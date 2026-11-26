use crate::{ImageError, Row, TableMeta};
use std::fmt;

/// 固定元数据的有界行镜像；添加失败保持原记录和字节计数不变。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.TableRecords；行读取/原序列化尚未迁移。
#[derive(Clone, PartialEq, Eq)]
pub struct TableRecords {
    table_meta: TableMeta,
    rows: Vec<Row>,
    value_bytes: usize,
    row_limit: usize,
    byte_limit: usize,
}
impl TableRecords {
    /// 参数为不可再替换的完整元数据；返回默认1024行/8MiB负载预算的空记录。
    pub fn new(table_meta: TableMeta) -> Self {
        Self {
            table_meta,
            rows: Vec::new(),
            value_bytes: 0,
            row_limit: 1024,
            byte_limit: 8_388_608,
        }
    }
    /// 参数为元数据及下调后的正数行数/字节预算；返回空记录，上调硬上限返回错误。
    pub fn with_limits(
        table_meta: TableMeta,
        row_limit: usize,
        byte_limit: usize,
    ) -> Result<Self, ImageError> {
        if !(1..=1024).contains(&row_limit) || !(1..=8_388_608).contains(&byte_limit) {
            return Err(ImageError::Budget {
                limit: "1..1024 rows and 1..8MiB payload",
            });
        }
        Ok(Self {
            row_limit,
            byte_limit,
            ..Self::new(table_meta)
        })
    }
    /// 参数为已验证完整行；返回添加结果，schema或预算失败不修改原镜像。
    pub fn add(&mut self, row: Row) -> Result<(), ImageError> {
        if !row.matches_table(&self.table_meta) {
            return Err(ImageError::Invalid {
                reason: "row belongs to a different schema",
            });
        }
        let next_bytes =
            self.value_bytes
                .checked_add(row.value_bytes())
                .ok_or(ImageError::Budget {
                    limit: "image payload overflow",
                })?;
        if false || next_bytes > self.byte_limit {
            return Err(ImageError::Budget {
                limit: "image row/payload budget",
            });
        }
        self.rows.push(row);
        self.value_bytes = next_bytes;
        Ok(())
    }
    /// 无参数；返回固定表元数据。
    pub fn table_meta(&self) -> &TableMeta {
        &self.table_meta
    }
    /// 无参数；返回原表名。
    pub fn table_name(&self) -> &str {
        self.table_meta.name()
    }
    /// 无参数；返回有序完整行，不进行Rust排序或数据库collation推断。
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    /// 无参数；返回累计值负载字节数。
    pub const fn value_bytes(&self) -> usize {
        self.value_bytes
    }
}
impl fmt::Debug for TableRecords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableRecords")
            .field("table", &self.table_name())
            .field("rows", &self.rows.len())
            .field("value_bytes", &self.value_bytes)
            .finish_non_exhaustive()
    }
}
