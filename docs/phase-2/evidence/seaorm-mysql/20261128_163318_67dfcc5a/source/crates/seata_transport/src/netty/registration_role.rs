/// 单物理会话唯一的原生登记角色，不在同一通道偷偷合并TM/RM身份。
/// 对应 Java: NettyPoolKey.TransactionRole。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationRole {
    /// 事务管理器角色。
    Tm,
    /// 资源管理器角色。
    Rm,
}
