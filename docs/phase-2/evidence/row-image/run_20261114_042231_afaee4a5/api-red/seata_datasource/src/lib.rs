//! 数据源元数据与镜像公共契约，数据库方言由独立适配器负责。
mod metadata_error;
mod r#struct;
pub use metadata_error::MetadataError;
pub use r#struct::{ColumnMeta, ColumnType, TableMeta};
