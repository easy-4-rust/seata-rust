use seata_rm::BranchIdentity;
use std::fmt;
/// 已校验的原登记身份与完整报告版本；只供持久适配器使用，不作为业务完成证明。
/// 对应 Java: BusinessActionContextUtil#reportContext 的完整信封；Rust 持久版本扩展。
pub(crate) struct MySqlTccContextVersion {
    /// 原成功登记的完整身份，应用数据不随报告改写。
    pub(crate) original: BranchIdentity,
    /// 此版本的完整原报告载荷。
    pub(crate) application_data: String,
    /// 从零起连续、受预算限制的版本号。
    pub(crate) revision: u16,
}
impl fmt::Debug for MySqlTccContextVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlTccContextVersion")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}
