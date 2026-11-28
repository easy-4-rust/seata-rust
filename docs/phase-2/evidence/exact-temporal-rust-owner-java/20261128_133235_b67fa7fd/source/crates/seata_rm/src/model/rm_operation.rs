/// RM单次出站操作，登记响应丢失不产生虚构分支号或重放授权。
/// 对应 Java: AbstractResourceManager#branchRegister/branchReport/lockQuery。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmOperation {
    /// 向TC申请分支编号。
    BranchRegister,
    /// 向TC报告分支状态。
    BranchReport,
    /// 查询普通排他锁是否可用。
    LockQuery,
}
