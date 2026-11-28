use super::{FastjsonUndoFieldValue, JacksonUndoField, JacksonUndoFieldValue, TableRecordsError};
use seata_sqlparser::{JavaString, TableMeta};
use std::collections::HashMap;

type PrimaryKeyRow<'a> = HashMap<&'a str, &'a JacksonUndoField>;

/// 原 TableRecords，保留行顺序、空镜像子类和一次性附加的临时表元数据。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.TableRecords。
#[derive(Clone)]
pub struct JacksonUndoRecords {
    pub(super) table: Option<String>,
    pub(super) rows: Vec<Vec<JacksonUndoField>>,
    pub(super) empty: bool,
    table_meta: Option<TableMeta>,
}

impl PartialEq for JacksonUndoRecords {
    fn eq(&self, other: &Self) -> bool {
        // 临时元数据不进入 Undo 字节比较，也不要求反序列化恢复原元数据句柄。
        self.table == other.table && self.rows == other.rows && self.empty == other.empty
    }
}

impl JacksonUndoRecords {
    /// 参数为可空表名及有序行，返回普通原对象，临时元数据可另行附加。
    pub fn new(table: Option<String>, rows: Vec<Vec<JacksonUndoField>>) -> Self {
        Self {
            table,
            rows,
            empty: false,
            table_meta: None,
        }
    }

    /// 参数为可空表名，返回原 EmptyTableRecords 子类。
    pub fn empty(table: Option<String>) -> Self {
        Self {
            empty: true,
            ..Self::new(table, Vec::new())
        }
    }

    /// 参数为临时元数据，首次附加成功并同步表名，重复附加返回错误。
    /// 对应 Java: TableRecords#setTableMeta。
    pub fn set_table_meta(&mut self, meta: TableMeta) -> Result<(), TableRecordsError> {
        if self.table_meta.is_some() {
            return Err(TableRecordsError::MetadataAlreadySet);
        }
        self.table = meta
            .table_name()?
            .map(|name| {
                name.as_str()
                    .map(str::to_owned)
                    .ok_or(TableRecordsError::ColumnShape)
            })
            .transpose()?;
        self.table_meta = Some(meta);
        Ok(())
    }

    /// 无参数；返回临时表元数据，空镜像子类返回不支持操作错误。
    /// 对应 Java: TableRecords#getTableMeta。
    pub fn table_meta(&self) -> Result<Option<&TableMeta>, TableRecordsError> {
        if self.empty {
            return Err(TableRecordsError::EmptyRecords);
        }
        Ok(self.table_meta.as_ref())
    }

    /// 无参数；返回原记录行数，空镜像子类返回零。
    /// 对应 Java: TableRecords#size。
    pub fn size(&self) -> usize {
        self.rows.len()
    }

    /// 参数为一行的有序字段，附加成功返回空值；空镜像子类拒绝附加。
    /// 对应 Java: TableRecords#add。
    pub fn add(&mut self, row: Vec<JacksonUndoField>) -> Result<(), TableRecordsError> {
        if self.empty {
            return Err(TableRecordsError::EmptyRecords);
        }
        if self.rows.len() >= 1024 || row.len() > 128 {
            return Err(TableRecordsError::CollectionLimit);
        }
        self.rows.push(row);
        Ok(())
    }

    /// 无参数；按原行序返回每行的主键字段映射，空镜像子类返回空集合。
    /// 对应 Java: TableRecords#pkRows。
    pub fn pk_rows(&self) -> Result<Vec<PrimaryKeyRow<'_>>, TableRecordsError> {
        if self.empty {
            return Ok(Vec::new());
        }
        let meta = self
            .table_meta()?
            .ok_or(TableRecordsError::MetadataMissing)?;
        let keys = meta.primary_key_map()?;
        Ok(self
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .filter(|field| keys.contains_key(Some(&JavaString::from(field.name()))))
                    .map(|field| (field.name(), field))
                    .collect()
            })
            .collect())
    }

    /// 参数为元数据、按 JDBC 顺序排列的原列名及各行已读取的 JDBC 值，返回原记录。
    /// 对应 Java: TableRecords#buildRecords 的字段投影与 Blob/Clob 包装步骤。
    pub fn build_records(
        meta: TableMeta,
        columns: &[&str],
        rows: impl IntoIterator<Item = Vec<JacksonUndoFieldValue>>,
    ) -> Result<Self, TableRecordsError> {
        if columns.len() > 128 {
            return Err(TableRecordsError::CollectionLimit);
        }
        let keys = meta.case_insensitive_pks()?;
        let mut records = Self::new(None, Vec::new());
        records.set_table_meta(meta.clone())?;
        let labels = columns
            .iter()
            .map(|name| JavaString::from(*name))
            .collect::<Vec<_>>();
        for row in rows {
            if row.len() != labels.len() {
                return Err(TableRecordsError::ColumnShape);
            }
            let fields = labels
                .iter()
                .zip(row)
                .map(|(label, value)| {
                    let column = meta
                        .column_meta(Some(label))?
                        .ok_or(TableRecordsError::UnknownColumn)?;
                    let name = column
                        .column_name()?
                        .and_then(|name| name.as_str().map(str::to_owned))
                        .ok_or(TableRecordsError::ColumnShape)?;
                    let code = column.data_type()?;
                    let primary = keys.contains(Some(label))?;
                    let value = Self::wrap_jdbc_value(code, value)?;
                    Ok(JacksonUndoField::new(name, code, primary, value))
                })
                .collect::<Result<Vec<_>, TableRecordsError>>()?;
            records.add(fields)?;
        }
        Ok(records)
    }

    fn wrap_jdbc_value(
        code: i32,
        value: JacksonUndoFieldValue,
    ) -> Result<JacksonUndoFieldValue, TableRecordsError> {
        use FastjsonUndoFieldValue::{Binary, Null, Text};
        use JacksonUndoFieldValue::{Common, OffsetDateTime, SerialBlob, SerialClob};
        match (code, value) {
            (2004, Common(Binary(bytes))) => Ok(SerialBlob(bytes)),
            (2005 | 2011, Common(Text(text))) => Ok(SerialClob(text)),
            (2004, value @ SerialBlob(_))
            | (2005 | 2011, value @ SerialClob(_))
            | (2014, value @ OffsetDateTime(_))
            | (_, value @ Common(Null)) => Ok(value),
            (2004 | 2005 | 2011 | 2014, _) => Err(TableRecordsError::ColumnShape),
            (_, value) => Ok(value),
        }
    }

    /// 无参数；返回原镜像表名。
    pub fn table_name(&self) -> Option<&str> {
        self.table.as_deref()
    }

    /// 无参数；返回原行列表的只读视图。
    pub fn rows(&self) -> &[Vec<JacksonUndoField>] {
        &self.rows
    }

    /// 无参数；返回是否来自原 EmptyTableRecords 子类。
    pub fn is_empty_records(&self) -> bool {
        self.empty
    }
}
