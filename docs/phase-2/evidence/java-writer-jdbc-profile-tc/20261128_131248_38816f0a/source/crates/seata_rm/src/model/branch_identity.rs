use crate::{BranchKey, RmError};
use seata_transport::ResourceKey;
use std::fmt;

/// 原始分支身份及应用数据；高层校验后不做资源名、XID或可选值规范化。
/// 对应 Java: AbstractBranchEndRequest各字段与ResourceManager分派参数。
#[derive(Clone, PartialEq, Eq)]
pub struct BranchIdentity {
    xid: String,
    branch_id: i64,
    resource: ResourceKey,
    application_data: Option<String>,
}
impl BranchIdentity {
    /// 参数为原始XID、正i64分支编号、资源键及可选应用数据；返回可持久化身份。
    /// 空应用数据必须用None；Some空串会在Java解码时丢失区别，因此拒绝。
    pub fn new(
        xid: String,
        branch_id: i64,
        resource: ResourceKey,
        application_data: Option<String>,
    ) -> Result<Self, RmError> {
        Self::validate_fields(
            &xid,
            branch_id,
            resource.resource_id(),
            application_data.as_deref(),
        )?;
        Ok(Self {
            xid,
            branch_id,
            resource,
            application_data,
        })
    }
    /// 借用协议字段先校验边界，再复制已接受的原身份；超限应用数据不会先被克隆。
    /// 参数保留原XID、分支号、模式、资源及可选应用数据，返回高层有效身份。
    pub(crate) fn from_fields(
        xid: &str,
        branch_id: i64,
        kind: seata_protocol::BranchType,
        resource: &str,
        application_data: Option<&str>,
    ) -> Result<Self, RmError> {
        Self::validate_fields(xid, branch_id, resource, application_data)?;
        let resource = ResourceKey::new(kind, resource.into()).map_err(|_| RmError::Invalid {
            reason: "invalid callback resource",
        })?;
        Ok(Self {
            xid: xid.into(),
            branch_id,
            resource,
            application_data: application_data.map(str::to_owned),
        })
    }
    fn validate_fields(
        xid: &str,
        branch_id: i64,
        resource: &str,
        application_data: Option<&str>,
    ) -> Result<(), RmError> {
        if xid.is_empty()
            || xid.len() > i16::MAX as usize
            || xid.trim() != xid
            || xid.chars().any(char::is_control)
            || branch_id <= 0
        {
            return Err(RmError::Invalid {
                reason: "XID must be bounded and nonempty; branch ID must be positive",
            });
        }
        if resource.len() > i16::MAX as usize
            || application_data
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > 1_048_576)
        {
            return Err(RmError::Invalid {
                reason:
                    "resource/application data exceeds bounds or has ambiguous empty optional data",
            });
        }
        Ok(())
    }
    /// 返回原始XID借用，不用于日志展示。
    pub fn xid(&self) -> &str {
        &self.xid
    }
    /// 返回精确有符号64位分支编号。
    pub const fn branch_id(&self) -> i64 {
        self.branch_id
    }
    /// 返回模式与精确资源ID组成的键。
    pub fn resource(&self) -> &ResourceKey {
        &self.resource
    }
    /// 返回原始可选应用数据借用，不用于日志展示。
    pub fn application_data(&self) -> Option<&str> {
        self.application_data.as_deref()
    }
    /// 返回XID和BranchId组合的去重键。
    pub fn key(&self) -> BranchKey {
        BranchKey {
            xid: self.xid.clone(),
            branch_id: self.branch_id,
        }
    }
}
impl fmt::Debug for BranchIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BranchIdentity")
            .field("xid_bytes", &self.xid.len())
            .field("branch_id", &self.branch_id)
            .field("resource", &self.resource)
            .field(
                "application_data_bytes",
                &self.application_data.as_ref().map(String::len),
            )
            .finish()
    }
}
