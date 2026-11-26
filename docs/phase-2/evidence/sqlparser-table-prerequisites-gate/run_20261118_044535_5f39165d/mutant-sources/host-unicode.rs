//! Rust扩展：固定原JDK17的Character简单大小写数据；不以宿主Rust的Unicode版本代替。
//! 对应 Java: java.lang.Character#toUpperCase(int)/toLowerCase(int)，供String比较器使用。

const DATA: &[u8] = include_bytes!("data/java17_character_case.bin");
const RECORD_BYTES: usize = 12;

fn integer(offset: usize) -> u32 {
    u32::from_be_bytes([
        DATA[offset],
        DATA[offset + 1],
        DATA[offset + 2],
        DATA[offset + 3],
    ])
}

fn mapped(code_point: u32, field: usize) -> u32 {
    let mut low = 0;
    let mut high = DATA.len() / RECORD_BYTES;
    while low < high {
        let middle = low + (high - low) / 2;
        let offset = middle * RECORD_BYTES;
        match integer(offset).cmp(&code_point) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return integer(offset + field),
        }
    }
    code_point
}

/// 将code_point按固定JDK17简单映射转为大写，返回单个码点；不做String展开或Locale转换。
/// 对应 Java: java.lang.Character#toUpperCase(int)
pub(crate) fn simple_upper(code_point: u32) -> u32 {
    char::from_u32(code_point).and_then(|ch| ch.to_uppercase().next()).map(u32::from).unwrap_or(code_point)
}

/// 将code_point按固定JDK17简单映射转为小写，返回单个码点；不做String展开或Locale转换。
/// 对应 Java: java.lang.Character#toLowerCase(int)
pub(crate) fn simple_lower(code_point: u32) -> u32 {
    char::from_u32(code_point).and_then(|ch| ch.to_lowercase().next()).map(u32::from).unwrap_or(code_point)
}
