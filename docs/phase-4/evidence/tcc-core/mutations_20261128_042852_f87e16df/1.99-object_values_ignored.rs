use crate::TccError;
use serde_json::value::RawValue;
use std::collections::BTreeMap;

/// 固定 Java JSON Provider 产生的值对象相等性，仅比较，不重编码或替换业务原值。
/// 对应 Java: BusinessActionContext.TrackedActionContextMap 的 Objects#equals。
pub(crate) struct ActionContextValueEquality;
impl ActionContextValueEquality {
    /// 参数为已受预算限制的原 JSON 值；返回 Java 值对象是否相等，不授予报告成功语义。
    pub(crate) fn equal(left: &RawValue, right: &RawValue) -> Result<bool, TccError> {
        let (left, right) = (left.get(), right.get());
        if left == right || left.starts_with('{') {
            return Ok(true);
        }
        let invalid = |_| TccError::InvalidContext;
        match (left.as_bytes()[0], right.as_bytes()[0]) {
            (b'"', b'"') => Ok(serde_json::from_str::<String>(left).map_err(invalid)?
                == serde_json::from_str::<String>(right).map_err(invalid)?),
            (b'{', b'{') => {
                let left: BTreeMap<String, Box<RawValue>> =
                    serde_json::from_str(left).map_err(invalid)?;
                let right: BTreeMap<String, Box<RawValue>> =
                    serde_json::from_str(right).map_err(invalid)?;
                if left.len() != right.len() {
                    return Ok(false);
                }
                for (key, value) in left {
                    let Some(other) = right.get(&key) else {
                        return Ok(false);
                    };
                    if !Self::equal(&value, other)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (b'[', b'[') => {
                let left: Vec<Box<RawValue>> = serde_json::from_str(left).map_err(invalid)?;
                let right: Vec<Box<RawValue>> = serde_json::from_str(right).map_err(invalid)?;
                if left.len() != right.len() {
                    return Ok(false);
                }
                for (value, other) in left.iter().zip(right.iter()) {
                    if !Self::equal(value, other)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (b'-' | b'0'..=b'9', b'-' | b'0'..=b'9') => Self::number_equal(left, right),
            _ => Ok(false),
        }
    }
    fn number_equal(left: &str, right: &str) -> Result<bool, TccError> {
        let floating = |v: &str| v.bytes().any(|b| matches!(b, b'.' | b'e' | b'E'));
        match (floating(left), floating(right)) {
            (true, true) => {
                // 原 Provider 使用 Double.equals，包括有符号零及溢出到无穷的比较。
                // 转换结果绝不回写 RawValue，也不能据此证明两个原载荷字节相同。
                let left = left.parse::<f64>().map_err(|_| TccError::InvalidContext)?;
                let right = right.parse::<f64>().map_err(|_| TccError::InvalidContext)?;
                Ok(left.to_bits() == right.to_bits())
            }
            (false, false) => {
                let family = |v: &str| {
                    if v.parse::<i32>().is_ok() {
                        0
                    } else if v.parse::<i64>().is_ok() {
                        1
                    } else {
                        2
                    }
                };
                if family(left) != family(right) {
                    return Ok(false);
                }
                Ok(match (left.parse::<i64>(), right.parse::<i64>()) {
                    (Ok(left), Ok(right)) => left == right,
                    _ => left == right,
                })
            }
            _ => Ok(false),
        }
    }
}
