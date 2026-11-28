/// 全局事务传播策略，嵌套作用域不得改写外层角色或身份。
/// 对应 Java: org.apache.seata.tm.api.transaction.Propagation
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Propagation {
    /// 有事务则参与，否则新建。
    #[default]
    Required,
    /// 暂停外层，始终新建独立事务。
    RequiresNew,
    /// 暂停外层，以无事务上下文执行。
    NotSupported,
    /// 有事务则参与，否则无事务执行。
    Supports,
    /// 必须参与已存在事务，否则在执行前拒绝。
    Mandatory,
    /// 必须无事务，否则在执行前拒绝。
    Never,
}
