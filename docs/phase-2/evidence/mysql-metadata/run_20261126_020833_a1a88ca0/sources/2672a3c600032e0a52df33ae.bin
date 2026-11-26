use super::{
    dmdb_timestamp::DMDB_TIMESTAMP, offset_date_time::OFFSET_DATE_TIME, DmdbTimestamp,
    FastjsonUndoFieldValue, OffsetDateTime,
};

/// Jackson ObjectMapper 的 JDBC 字段装箱值，保留 Java 类型区别。
/// 对应 Java: org.apache.seata.rm.datasource.sql.struct.Field#getValue。
#[derive(Clone)]
pub enum JacksonUndoFieldValue {
    /// 两种序列化器共用的装箱值。
    Common(FastjsonUndoFieldValue),
    /// 有限 Java Double，保留二进制值及零的符号。
    Double(f64),
    /// java.sql.Timestamp 的 Unix 秒和秒内纳秒。
    Timestamp {
        /// 向下取整的 Unix 秒。
        epoch_seconds: i64,
        /// 秒内纳秒，范围为 0 到 999999999。
        nanos: u32,
    },
    /// 在 UTC 配置下映射的 java.time.LocalDateTime。
    LocalDateTimeUtc {
        /// 将原本地时间视为 UTC 后的 Unix 秒。
        epoch_seconds: i64,
        /// 原本地时间的秒内纳秒。
        nanos: u32,
    },
    /// 达梦原时间值及 UTC 时区来源。
    DmdbTimestamp(DmdbTimestamp),
    /// java.time.OffsetDateTime，保留 JDBC 对象的原偏移。
    OffsetDateTime(OffsetDateTime),
    /// javax.sql.rowset.serial.SerialBlob 的字节。原编码器要求内容非空。
    SerialBlob(Vec<u8>),
    /// javax.sql.rowset.serial.SerialClob 的字符。
    SerialClob(String),
}

impl JacksonUndoFieldValue {
    /// 无参数；返回对应的 Java 装箱类名称。空值返回 "null"。
    pub const fn java_class(&self) -> &'static str {
        match self {
            Self::Common(value) => match value {
                FastjsonUndoFieldValue::Null => "null",
                FastjsonUndoFieldValue::Boolean(_) => "java.lang.Boolean",
                FastjsonUndoFieldValue::Integer(_) => "java.lang.Integer",
                FastjsonUndoFieldValue::Long(_) => "java.lang.Long",
                FastjsonUndoFieldValue::Unsigned(_) => "java.math.BigInteger",
                FastjsonUndoFieldValue::Decimal(_) => "java.math.BigDecimal",
                FastjsonUndoFieldValue::Text(_) => "java.lang.String",
                FastjsonUndoFieldValue::Binary(_) => "[B",
                FastjsonUndoFieldValue::SerialArray(_) => super::serial_array::SERIAL_ARRAY,
            },
            Self::Double(_) => "java.lang.Double",
            Self::Timestamp { .. } => "java.sql.Timestamp",
            Self::LocalDateTimeUtc { .. } => "java.time.LocalDateTime",
            Self::DmdbTimestamp(_) => DMDB_TIMESTAMP,
            Self::OffsetDateTime(_) => OFFSET_DATE_TIME,
            Self::SerialBlob(_) => "javax.sql.rowset.serial.SerialBlob",
            Self::SerialClob(_) => "javax.sql.rowset.serial.SerialClob",
        }
    }
}

impl PartialEq for JacksonUndoFieldValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Common(left), Self::Common(right)) => left == right,
            (Self::Double(left), Self::Double(right)) => {
                // Java Double.equals 区分正负零，并把 NaN 位模式归为同一值。
                left.to_bits() == right.to_bits() || left.is_nan() && right.is_nan()
            }
            (
                Self::Timestamp {
                    epoch_seconds: left,
                    nanos: left_nanos,
                },
                Self::Timestamp {
                    epoch_seconds: right,
                    nanos: right_nanos,
                },
            )
            | (
                Self::LocalDateTimeUtc {
                    epoch_seconds: left,
                    nanos: left_nanos,
                },
                Self::LocalDateTimeUtc {
                    epoch_seconds: right,
                    nanos: right_nanos,
                },
            ) => (left, left_nanos) == (right, right_nanos),
            (Self::DmdbTimestamp(left), Self::DmdbTimestamp(right)) => left == right,
            (Self::OffsetDateTime(left), Self::OffsetDateTime(right)) => left == right,
            (Self::SerialBlob(left), Self::SerialBlob(right)) => left == right,
            (Self::SerialClob(left), Self::SerialClob(right)) => left == right,
            _ => false,
        }
    }
}
