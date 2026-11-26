use crate::{ColumnType, MetadataError, TableMeta};

/// 不可变列元数据，保留物理顺序、主键顺序和精确维度。
/// 对应 Java: org.apache.seata.sqlparser.struct.ColumnMeta。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    name: String,
    kind: ColumnType,
    nullable: bool,
    ordinal: u32,
    primary_key_ordinal: Option<u32>,
    char_octet_length: Option<u64>,
    numeric_precision: Option<u32>,
    numeric_scale: Option<u32>,
}
impl ColumnMeta {
    /// 参数为原列名、值域、空值/物理/主键顺序和JDBC维度；返回已验证的不可变列。
    /// 八字段对应同一原子元数据记录，保留平面构造以免不完整Builder状态泄漏。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        kind: ColumnType,
        nullable: bool,
        ordinal: u32,
        primary_key_ordinal: Option<u32>,
        char_octet_length: Option<u64>,
        numeric_precision: Option<u32>,
        numeric_scale: Option<u32>,
    ) -> Result<Self, MetadataError> {
        TableMeta::validate_identifier(&name)?;
        if ordinal == 0
            || primary_key_ordinal == Some(0)
            || (primary_key_ordinal.is_some() && nullable)
        {
            return Err(MetadataError::Invalid {
                reason: "column/primary ordinal or nullability",
            });
        }
        if kind == ColumnType::Decimal
            && !matches!((numeric_precision,numeric_scale), (Some(p),Some(s)) if p>0 && p<=65 && s<=30 && s<=p)
        {
            return Err(MetadataError::Invalid {
                reason: "DECIMAL precision/scale",
            });
        }
        if char_octet_length == Some(0) {
            return Err(MetadataError::Invalid {
                reason: "zero character/binary byte length",
            });
        }
        Ok(Self {
            name,
            kind,
            nullable,
            ordinal,
            primary_key_ordinal,
            char_octet_length,
            numeric_precision,
            numeric_scale,
        })
    }
    /// 无参数；返回原列名，不改变大小写。
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 无参数；返回精确值域。
    pub const fn kind(&self) -> ColumnType {
        self.kind
    }
    /// 无参数；返回列是否可为NULL。
    pub const fn nullable(&self) -> bool {
        self.nullable
    }
    /// 无参数；返回从1开始的物理列位置。
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    /// 无参数；返回该列在主键索引中的位置，非主键为None。
    pub const fn primary_key_ordinal(&self) -> Option<u32> {
        self.primary_key_ordinal
    }
    /// 无参数；返回数据库声明的字符/二进制最大字节长度。
    pub const fn char_octet_length(&self) -> Option<u64> {
        self.char_octet_length
    }
    /// 无参数；返回数据库声明的数值精度。
    pub const fn numeric_precision(&self) -> Option<u32> {
        self.numeric_precision
    }
    /// 无参数；返回数据库声明的小数位数。
    pub const fn numeric_scale(&self) -> Option<u32> {
        self.numeric_scale
    }
}
