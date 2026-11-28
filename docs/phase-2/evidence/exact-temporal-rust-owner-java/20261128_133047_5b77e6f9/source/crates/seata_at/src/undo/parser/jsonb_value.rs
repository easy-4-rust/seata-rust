use std::collections::BTreeMap;

/// Fastjson2 JSONB 的有界中间值，区分装箱整型与原类标签。
/// 对应 Java: Fastjson2 JSONReaderJSONB 的受支持值域。
pub(super) enum JsonbValue {
    Null,
    Bool(bool),
    Int(i32),
    Long(i64),
    BigInteger(u64),
    Decimal(seata_datasource::DecimalValue),
    Binary(Vec<u8>),
    Text(String),
    List(Vec<Self>),
    Object(Option<String>, BTreeMap<String, Self>),
}
