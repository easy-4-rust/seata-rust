use super::business_action_context::validate_budget;
use crate::TccError;
use serde_json::value::RawValue;

/// 将业务复合 JSON 值转换成可在二阶段读取的 JSON 字符串，标量保留原字节。
/// 对应 Java: org.apache.seata.integration.tx.api.interceptor.ActionContextUtil#handleActionContext。
/// Rust 输入是已验证 RawValue，不添加 Java 运行时类元数据，不重编码原数值精度。
#[derive(Debug)]
pub struct ActionContextUtil;
impl ActionContextUtil {
    /// 参数为原 JSON 值；返回标量原值或保存复合 JSON 原字节的字符串，超预算返回错误。
    pub fn handle_action_context(value: &RawValue) -> Result<Box<RawValue>, TccError> {
        validate_budget(value.get())?;
        if false {
            let encoded =
                serde_json::to_string(value.get()).map_err(|_| TccError::InvalidContext)?;
            validate_budget(&encoded)?;
            RawValue::from_string(encoded).map_err(|_| TccError::InvalidContext)
        } else {
            Ok(value.to_owned())
        }
    }
}
