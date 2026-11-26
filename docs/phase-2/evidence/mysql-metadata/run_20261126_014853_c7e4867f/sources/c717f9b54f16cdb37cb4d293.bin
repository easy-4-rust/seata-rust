use crate::MetadataError;
use std::{
    collections::HashMap,
    fmt,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, OnceLock, Weak},
};

type InternPool = (HashMap<Vec<u16>, Weak<[u16]>>, usize);
static INTERN_POOL: OnceLock<Mutex<InternPool>> = OnceLock::new();

/// Rust 扩展：不可变 Java 字符串的无损表示，保留孤立代理码元及 NUL。
/// 对应 Java: java.lang.String 的 UTF-16 表示契约；不计入 sqlparser 的31对象分母。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaString {
    units: Arc<[u16]>,
    utf8: Option<Arc<str>>,
}

impl JavaString {
    /// 判断本字符串是否为空或全部由固定 JDK 的空白码元组成。
    /// 对应 Java: org.apache.seata.common.util.StringUtils#isBlank 的非空引用路径。
    pub fn is_java_blank(&self) -> bool {
        self.units.iter().all(|unit| {
            matches!(unit,
                0x0009..=0x000d | 0x001c..=0x0020 | 0x1680 |
                0x2000..=0x2006 | 0x2008..=0x200a | 0x2028..=0x2029 | 0x205f | 0x3000
            )
        })
    }

    /// 返回去掉首尾不大于 U+0020 码元的字符串；无需裁剪时保留原引用身份。
    /// 对应 Java: String#trim。
    pub fn java_trim(&self) -> Self {
        let units = self.trimmed_utf16();
        if units.len() == self.units.len() {
            self.clone()
        } else {
            Self::from_utf16(units.to_vec())
        }
    }

    /// 返回裁剪后的借用码元视图。
    /// 对应 Java: String#trim 的 Rust 内部零分配视图。
    pub(crate) fn trimmed_utf16(&self) -> &[u16] {
        let start = self.units.iter().take_while(|&&u| u <= 0x20).count();
        let rest = &self.units[start..];
        let trailing = rest.iter().rev().take_while(|&&u| u <= 0x20).count();
        &rest[..rest.len() - trailing]
    }

    /// 判断 other 是否共享同一组原始码元，供保留引用身份的桥接调用使用。
    /// 对应 Java: String 引用的 ==。
    pub fn same_storage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.units, &other.units)
    }

    /// 对 units 使用进程内弱驻留表，返回当前可达的规范字符串。
    /// 对应 Java: String#intern；Rust 驻留表定期清理已释放的条目。
    pub(crate) fn intern_utf16(units: Vec<u16>) -> Result<Self, MetadataError> {
        let mut pool = INTERN_POOL
            .get_or_init(|| Mutex::new((HashMap::new(), 256)))
            .lock()
            .map_err(|_| MetadataError::LockPoisoned("JavaString.intern"))?;
        if let Some(existing) = pool.0.get(&units).and_then(Weak::upgrade) {
            let utf8 = String::from_utf16(&existing).ok().map(Arc::from);
            return Ok(Self {
                units: existing,
                utf8,
            });
        }
        if pool.0.len() >= pool.1 {
            pool.0.retain(|_, value| value.strong_count() != 0);
            pool.1 = pool.0.len().saturating_mul(2).max(256);
        }
        let value = Self::from_utf16(units.clone());
        pool.0.insert(units, Arc::downgrade(&value.units));
        Ok(value)
    }

    /// 保存 units 的全部 UTF-16 码元并返回字符串；非法代理序列也原样保留。
    /// 对应 Java: String(char[]) 的码元复制；Rust表示扩展。
    pub fn from_utf16(units: Vec<u16>) -> Self {
        let utf8 = String::from_utf16(&units).ok().map(Arc::from);
        Self {
            units: units.into(),
            utf8,
        }
    }

    /// 返回原 UTF-16 码元切片；无参数，不进行替换或 Unicode 归一化。
    /// 对应 Java: String#toCharArray 的只读视图；Rust表示扩展。
    pub fn as_utf16(&self) -> &[u16] {
        &self.units
    }

    /// 返回有效 Unicode 的 UTF-8 视图；无参数，孤立代理码元返回 None。
    /// 对应 Java: String 的显式UTF-8便利视图；Rust扩展，不替代原始码元访问。
    pub fn as_str(&self) -> Option<&str> {
        self.utf8.as_deref()
    }

    /// 返回原 Java String.hashCode 的有符号32位结果；无参数，溢出按补码回绕。
    /// 对应 Java: String#hashCode
    pub fn java_hash_code(&self) -> i32 {
        self.units.iter().fold(0_i32, |hash, unit| {
            hash.wrapping_mul(31).wrapping_add(i32::from(*unit))
        })
    }

    /// 与other按原JDK17的非Locale敏感比较器比较，返回原有符号差值；保留孤立/配对代理码元。
    /// 对应 Java: String#compareToIgnoreCase和String.CASE_INSENSITIVE_ORDER；不采用Rust Unicode版本。
    pub fn java_compare_to_ignore_case(&self, other: &Self) -> i32 {
        let left = self.as_utf16();
        let right = other.as_utf16();
        let latin = left.iter().all(|unit| *unit <= 0xff) || right.iter().all(|unit| *unit <= 0xff);
        let mut a = 0;
        let mut b = 0;
        while a < left.len() && b < right.len() {
            let x = u32::from(left[a]);
            let y = u32::from(right[b]);
            if x != y && compare_code_points(x, y) != 0 {
                // JDK只在两个字符串均为UTF16且初始码元比较不同后，回看/前看补充平面代理对。
                let (x, advance_a) = if latin {
                    (x, false)
                } else {
                    code_point_including(left, a)
                };
                let (y, advance_b) = if latin {
                    (y, false)
                } else {
                    code_point_including(right, b)
                };
                if advance_a {
                    a += 1;
                }
                if advance_b {
                    b += 1;
                }
                let difference = compare_code_points(x, y);
                if difference != 0 {
                    return difference;
                }
            }
            a += 1;
            b += 1;
        }
        (left.len() as i32).wrapping_sub(right.len() as i32)
    }
}

fn compare_code_points(left: u32, right: u32) -> i32 {
    let left = crate::java_case_mapping::simple_upper(left);
    let right = crate::java_case_mapping::simple_upper(right);
    if left == right {
        return 0;
    }
    let left = crate::java_case_mapping::simple_lower(left);
    let right = crate::java_case_mapping::simple_lower(right);
    left as i32 - right as i32
}

fn code_point_including(units: &[u16], index: usize) -> (u32, bool) {
    let current = units[index];
    if (0xd800..=0xdbff).contains(&current) && index + 1 < units.len() {
        let next = units[index + 1];
        if (0xdc00..=0xdfff).contains(&next) {
            return (
                0x10000 + (u32::from(current) - 0xd800) * 0x400 + u32::from(next) - 0xdc00,
                true,
            );
        }
    }
    if (0xdc00..=0xdfff).contains(&current) && index > 0 {
        let previous = units[index - 1];
        if (0xd800..=0xdbff).contains(&previous) {
            return (
                0x10000 + (u32::from(previous) - 0xd800) * 0x400 + u32::from(current) - 0xdc00,
                false,
            );
        }
    }
    (u32::from(current), false)
}

impl From<&str> for JavaString {
    fn from(value: &str) -> Self {
        Self {
            units: value.encode_utf16().collect::<Vec<_>>().into(),
            utf8: Some(Arc::from(value)),
        }
    }
}

impl From<String> for JavaString {
    fn from(value: String) -> Self {
        Self {
            units: value.encode_utf16().collect::<Vec<_>>().into(),
            utf8: Some(value.into()),
        }
    }
}

impl Hash for JavaString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.units.hash(state);
    }
}

impl fmt::Display for JavaString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Rust文本输出不能承载孤立代理码元；诊断明确转义，不悄悄替换为 U+FFFD。
        for decoded in char::decode_utf16(self.units.iter().copied()) {
            match decoded {
                Ok(character) => write!(f, "{character}")?,
                Err(error) => write!(f, "\\u{:04x}", error.unpaired_surrogate())?,
            }
        }
        Ok(())
    }
}
