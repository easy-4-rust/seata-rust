use crate::TccError;
use serde_json::value::RawValue;
use std::{collections::BTreeMap, fmt};
/// 原 TCC 上下文；业务 JSON 值保留原字节，不经 f64 或业务日志转换。
/// 对应 Java: org.apache.seata.rm.tcc.api.BusinessActionContext 和 BusinessActionContextUtil#getBusinessActionContext。
#[derive(Clone)]
pub struct BusinessActionContext {
    xid: Option<String>,
    branch_id: i64,
    action_name: Option<String>,
    action_context: BTreeMap<String, Box<RawValue>>,
}
impl BusinessActionContext {
    /// 最大原应用 JSON 字节数；超过预算在解析和复制前拒绝。
    pub const MAX_CONTEXT_BYTES: usize = 2_097_152;
    /// 最大 JSON 容器嵌套层数，包含外层信封。
    pub const MAX_JSON_DEPTH: usize = 64;
    /// 参数为原可选 XID、原 long 分支号、原可选资源名和应用 JSON；返回值对象，不授予 TC 权限。
    pub fn new(
        xid: Option<String>,
        branch_id: i64,
        action_name: Option<String>,
        application_data: Option<&str>,
    ) -> Result<Self, TccError> {
        let action_context = match application_data {
            None => BTreeMap::new(),
            Some(data) => {
                validate_budget(data)?;
                if data.chars().all(java_whitespace) {
                    BTreeMap::new()
                } else {
                    let mut envelope: BTreeMap<String, Box<RawValue>> =
                        serde_json::from_str(data).map_err(|_| TccError::InvalidContext)?;
                    match envelope.remove("actionContext") {
                        None => BTreeMap::new(),
                        Some(raw) if raw.get() == "null" => BTreeMap::new(),
                        Some(raw) => serde_json::from_str(raw.get())
                            .map_err(|_| TccError::ContextNotObject)?,
                    }
                }
            }
        };
        Ok(Self {
            xid,
            branch_id,
            action_name,
            action_context,
        })
    }
    /// 无参数；返回原可选 XID，不作规范化。
    pub fn xid(&self) -> Option<&str> {
        self.xid.as_deref()
    }
    /// 无参数；返回精确原 long 分支号。
    pub const fn branch_id(&self) -> i64 {
        self.branch_id
    }
    /// 无参数；返回原可选资源名，不允许上下文覆盖。
    pub fn action_name(&self) -> Option<&str> {
        self.action_name.as_deref()
    }
    /// 参数为业务键；返回已验证的原 JSON 值，不转换数值精度。
    pub fn get_action_context(&self, key: &str) -> Option<&RawValue> {
        self.action_context.get(key).map(Box::as_ref)
    }
    /// 无参数；返回只读业务参数映射。动态更新及报告须由正式资源层负责。
    pub fn action_context(&self) -> &BTreeMap<String, Box<RawValue>> {
        &self.action_context
    }
    /// 无参数；仅原 JSON 布尔 true 启用 Fence，不接受字符串或数字。
    pub fn use_common_fence(&self) -> bool {
        self.get_action_context("useTCCFence")
            .is_some_and(|value| value.get() == "true")
    }
    /// 无参数；返回 Java actionContext 信封，保留每个原业务值字节。
    pub fn to_application_data(&self) -> Result<String, TccError> {
        let envelope = BTreeMap::from([("actionContext", &self.action_context)]);
        let encoded = serde_json::to_string(&envelope).map_err(|_| TccError::InvalidContext)?;
        validate_budget(&encoded)?;
        Ok(encoded)
    }
}
impl fmt::Debug for BusinessActionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BusinessActionContext")
            .field("has_xid", &self.xid.is_some())
            .field("branch_id", &self.branch_id)
            .field(
                "action_name_bytes",
                &self.action_name.as_ref().map(String::len),
            )
            .field("context_keys", &self.action_context.len())
            .finish_non_exhaustive()
    }
}

// Java Character.isWhitespace 与 Rust Unicode White_Space 在控制分隔符、不换行空格上不同。
fn java_whitespace(value: char) -> bool {
    matches!(value, '\u{001c}'..='\u{001f}')
        || (value.is_whitespace()
            && !matches!(value, '\u{0085}' | '\u{00a0}' | '\u{2007}' | '\u{202f}'))
}
// RawValue 不把参数解析为 f64；另在整个原信封上执行字节/结构预算，包含未知字段。
fn validate_budget(data: &str) -> Result<(), TccError> {
    if data.len() > BusinessActionContext::MAX_CONTEXT_BYTES {
        return Err(TccError::ContextBudget);
    }
    let (mut depth, mut string, mut escaped) = (0usize, false, false);
    for byte in data.bytes() {
        if string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                string = false;
            }
        } else {
            match byte {
                b'"' => string = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > BusinessActionContext::MAX_JSON_DEPTH {
                        return Err(TccError::ContextBudget);
                    }
                }
                b'}' | b']' => {
                    depth = depth.checked_sub(1).ok_or(TccError::InvalidContext)?;
                }
                _ => {}
            }
        }
    }
    if string || depth != 0 {
        return Err(TccError::InvalidContext);
    }
    Ok(())
}
