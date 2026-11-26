use thiserror::Error;

/// 原 TableRecords 对象的表元数据及结果集投影错误。
/// 对应 Java: TableRecords 的原异常边界与 Rust 集合预算。
#[derive(Debug, Error)]
pub enum TableRecordsError {
    /// 重复设置临时表元数据，对应 ShouldNeverHappenException。
    #[error("TableRecords metadata has already been set")]
    MetadataAlreadySet,
    /// 普通记录缺少临时元数据。
    #[error("TableRecords metadata is missing")]
    MetadataMissing,
    /// 空镜像子类拒绝修改或读取元数据，对应 UnsupportedOperationException。
    #[error("operation is unavailable for EmptyTableRecords")]
    EmptyRecords,
    /// JDBC 结果列不在缓存表元数据中，对应 TableMetaException。
    #[error("JDBC column is absent from TableRecords metadata")]
    UnknownColumn,
    /// 列名、行宽或返回值装箱不符合结果集形状。
    #[error("invalid JDBC TableRecords column shape")]
    ColumnShape,
    /// 结果集超出列数或行数边界。
    #[error("TableRecords collection limit exceeded")]
    CollectionLimit,
    /// 表元数据访问返回的底层错误。
    #[error(transparent)]
    Metadata(#[from] seata_sqlparser::MetadataError),
}
