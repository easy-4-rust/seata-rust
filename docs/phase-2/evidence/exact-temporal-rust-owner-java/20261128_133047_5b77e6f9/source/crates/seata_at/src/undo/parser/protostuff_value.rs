use std::collections::BTreeMap;

/// 保留分组中的首字段及同字段号的原顺序，供动态对象的类型判别。
/// 对应 Java: ObjectSchema 按首字段编号选择装箱值或委托。
#[derive(Default)]
pub(super) struct ProtostuffFields<'a> {
    pub(super) first: Option<u32>,
    pub(super) members: BTreeMap<u32, Vec<ProtostuffValue<'a>>>,
}

/// Protostuff 原 Undo 支持的二进制字段值。
/// 对应 Java: io.protostuff.ByteArrayInput 的线格式。
pub(super) enum ProtostuffValue<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Group(ProtostuffFields<'a>),
}
