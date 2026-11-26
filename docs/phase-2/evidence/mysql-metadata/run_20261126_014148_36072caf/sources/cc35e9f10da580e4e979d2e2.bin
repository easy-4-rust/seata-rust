use crate::{MetadataLocale, TableMeta};
use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// SQL Server 表元数据，保存表是否包含 IDENTITY 列并继承普通表元数据行为。
/// 对应 Java: org.apache.seata.sqlparser.struct.SqlServerTableMeta。
#[derive(Clone, Debug)]
pub struct SqlServerTableMeta {
    base: TableMeta,
    table_identify_existence: Arc<AtomicBool>,
}

impl SqlServerTableMeta {
    /// 用 locale 构造表元数据，初始 IDENTITY 标志为 false。
    /// 对应 Java: SqlServerTableMeta 无参构造；沿用 TableMeta 的显式 Locale 注入。
    pub fn new(locale: MetadataLocale) -> Self {
        Self {
            base: TableMeta::new(locale),
            table_identify_existence: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 返回表是否包含 IDENTITY 列。
    /// 对应 Java: SqlServerTableMeta#isTableIdentifyExistence。
    pub fn is_table_identify_existence(&self) -> bool {
        self.table_identify_existence.load(Ordering::Relaxed)
    }

    /// 设置表的 IDENTITY 标志，所有共享句柄均读取同一标志。
    /// 对应 Java: SqlServerTableMeta#setTableIdentifyExistence。
    pub fn set_table_identify_existence(&self, table_identify_existence: bool) {
        self.table_identify_existence
            .store(table_identify_existence, Ordering::Relaxed);
    }
}

impl Deref for SqlServerTableMeta {
    type Target = TableMeta;

    /// 返回同一对象的父类元数据视图。
    /// 对应 Java: SqlServerTableMeta 继承 TableMeta。
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
