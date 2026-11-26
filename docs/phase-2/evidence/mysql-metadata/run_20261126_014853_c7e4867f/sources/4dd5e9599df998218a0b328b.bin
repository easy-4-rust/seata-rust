use crate::JavaCompareTemporal;
use seata_datasource::DecimalValue;
use seata_sqlparser::{JavaString, MetadataError};

/// 可由 Java 对象比较入口读取的原值。不同装箱类型保持不同身份。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field#getValue。
#[derive(Clone, PartialEq, Eq)]
pub enum JavaCompareValue {
    /// 原空引用。
    Null,
    /// 原 Java UTF-16 字符串。
    Text(JavaString),
    /// 原 java.lang.Integer。
    Integer(i32),
    /// 原 java.lang.Long。
    Long(i64),
    /// 原 java.lang.Boolean。
    Boolean(bool),
    /// 原字节数组，以内容比较。
    Binary(Vec<u8>),
    /// 受支持的精确 BigDecimal 值，保留小数位数。
    Decimal(DecimalValue),
    /// 日期时间类及宿主提供的标准化相等比较表示。
    Temporal(JavaCompareTemporal, JavaString),
}

impl JavaCompareValue {
    /// 返回受支持值的 Java 字符串表示，用于原 Map 键。
    pub(crate) fn row_key(&self) -> Result<JavaString, MetadataError> {
        match self {
            Self::Text(value) => Ok(value.clone()),
            Self::Null => Ok("null".into()),
            Self::Integer(value) => Ok(value.to_string().into()),
            Self::Long(value) => Ok(value.to_string().into()),
            Self::Boolean(value) => Ok(value.to_string().into()),
            Self::Decimal(value) => Ok(value.java_big_decimal_text().as_ref().into()),
            Self::Temporal(_, _) => Err(MetadataError::InvalidCompatibilityData(
                "Java temporal map key representation",
            )),
            Self::Binary(_) => Err(MetadataError::InvalidCompatibilityData(
                "Java byte array key identity",
            )),
        }
    }
}
