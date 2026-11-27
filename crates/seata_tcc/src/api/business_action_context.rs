use super::action_context_value_equality::ActionContextValueEquality;
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
    tracking_enabled: bool,
    updated: Option<bool>,
    delay_report: Option<bool>,
}
impl BusinessActionContext {
    /// 最大原应用 JSON 字节数；超过预算在解析和复制前拒绝。
    pub const MAX_CONTEXT_BYTES: usize = 1_048_576;
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
            tracking_enabled: false,
            updated: None,
            delay_report: None,
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
    /// 无参数；返回只读业务参数映射；变更须通过受控方法，网络报告由正式资源层负责。
    pub fn action_context(&self) -> &BTreeMap<String, Box<RawValue>> {
        &self.action_context
    }
    /// 无参数；返回 Java 可选 updated 标志，不证明原参数字节相同或报告已送达。
    pub const fn get_updated(&self) -> Option<bool> {
        self.updated
    }
    /// 参数为原可选 updated；只设置本地 Java 状态，不确认网络或数据库结果。
    pub fn set_updated(&mut self, updated: Option<bool>) {
        self.updated = updated;
    }
    /// 无参数；返回原可选 delayReport；此核心对象不发送报告。
    pub const fn get_delay_report(&self) -> Option<bool> {
        self.delay_report
    }
    /// 参数为原可选延迟标志；不发送网络请求或清除已有变更。
    pub fn set_delay_report(&mut self, delay_report: Option<bool>) {
        self.delay_report = delay_report;
    }
    /// 无参数；初始化完成后开启业务映射的变更跟踪，不将既有条目标为更新。
    pub fn enable_action_context_tracking(&mut self) {
        self.tracking_enabled = true;
    }
    /// 参数为业务键及原 JSON 值；有界原子替换，返回旧值，直接 put 可存储 null。
    /// 对应 Java: BusinessActionContext.TrackedActionContextMap#put。
    pub fn put_action_context(
        &mut self,
        key: &str,
        value: &RawValue,
    ) -> Result<Option<Box<RawValue>>, TccError> {
        if key.len() > Self::MAX_CONTEXT_BYTES {
            return Err(TccError::ContextBudget);
        }
        validate_budget(value.get())?;
        let previous = self.action_context.get(key);
        let changed = match previous {
            None => true,
            Some(old) => !ActionContextValueEquality::equal(old, value)?,
        };
        let mut candidate = self.action_context.clone();
        let previous = candidate.insert(key.into(), value.to_owned());
        Self::validate_mapping(&candidate)?;
        self.action_context = candidate;
        if self.tracking_enabled && changed {
            self.updated = Some(true);
        }
        Ok(previous)
    }
    /// 参数为业务键及可选原 JSON 值；忽略 null，返回 Java 对象相等性判断的变更结果。
    /// 对应 Java: BusinessActionContext#addActionContext；不等同 util 的自动报告入口。
    pub fn add_action_context(
        &mut self,
        key: &str,
        value: Option<&RawValue>,
    ) -> Result<bool, TccError> {
        let Some(value) = value.filter(|v| v.get() != "null") else {
            return Ok(false);
        };
        if key.len() > Self::MAX_CONTEXT_BYTES {
            return Err(TccError::ContextBudget);
        }
        validate_budget(value.get())?;
        let changed = match self.action_context.get(key) {
            None => true,
            Some(old) => !ActionContextValueEquality::equal(old, value)?,
        };
        self.put_action_context(key, value)?;
        if changed {
            self.updated = Some(true);
        }
        Ok(changed)
    }
    /// 参数为业务键；返回实际删除的旧值，删除已有 null 也在跟踪开启时标记变更。
    pub fn remove_action_context(&mut self, key: &str) -> Option<Box<RawValue>> {
        let previous = self.action_context.remove(key);
        if self.tracking_enabled && previous.is_some() {
            self.updated = Some(true);
        }
        previous
    }
    /// 无参数；清空映射，仅在原映射非空且跟踪开启时标记变更。
    pub fn clear_action_context(&mut self) {
        if !self.action_context.is_empty() {
            self.action_context.clear();
            if self.tracking_enabled {
                self.updated = Some(true);
            }
        }
    }
    /// 参数为替代业务映射；原子预算校验后替换，保留跟踪开关和原 updated 标志。
    /// 对应 Java: BusinessActionContext#setActionContext，不自动标记框架侧替换。
    pub fn set_action_context(
        &mut self,
        values: BTreeMap<String, Box<RawValue>>,
    ) -> Result<(), TccError> {
        Self::validate_mapping(&values)?;
        self.action_context = values;
        Ok(())
    }
    fn validate_mapping(values: &BTreeMap<String, Box<RawValue>>) -> Result<(), TccError> {
        let mut bytes = 0usize;
        for (key, value) in values {
            bytes = bytes
                .checked_add(key.len())
                .and_then(|n| n.checked_add(value.get().len()))
                .ok_or(TccError::ContextBudget)?;
            if bytes > Self::MAX_CONTEXT_BYTES {
                return Err(TccError::ContextBudget);
            }
        }
        let envelope = BTreeMap::from([("actionContext", values)]);
        let encoded = serde_json::to_string(&envelope).map_err(|_| TccError::InvalidContext)?;
        validate_budget(&encoded)
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
            .field("updated", &self.updated)
            .field("delay_report", &self.delay_report)
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
