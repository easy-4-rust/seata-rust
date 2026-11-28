//! 数据源元数据与镜像公共契约，数据库方言由独立适配器负责。
mod image_error;
mod metadata_error;
mod r#struct;
pub use image_error::ImageError;
pub use metadata_error::MetadataError;
pub use r#struct::{
    CharacterSet, ColumnDomain, ColumnMeta, ColumnType, DecimalValue, Field, FieldValue,
    InternalTime, LocalDate, LocalDateTime, MySqlType, Row, TableMeta, TableRecords, Timestamp,
};
