use crate::{RegistrationAttempt, ResourceKey};
/// 一个所需typed资源的登记结果；后续尚未执行步骤仍被显式保留。
/// 对应 Java: RegisterCachedResources逐资源登记/错误汇总职责。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRegistrationResult {
    /// 精确资源键，Debug按ResourceKey策略隐藏标识内容。
    pub resource: ResourceKey,
    /// 当前步骤的分类和原始传输错误。
    pub attempt: RegistrationAttempt,
}
