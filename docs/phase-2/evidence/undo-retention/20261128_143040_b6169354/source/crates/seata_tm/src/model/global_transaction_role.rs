/// 全局事务角色；参与方没有全局二阶段决策权。
/// 对应 Java: org.apache.seata.tm.api.GlobalTransactionRole
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalTransactionRole {
    /// 本地新建事务的发起方。
    Launcher,
    /// 使用已存在XID的参与方。
    Participant,
}
