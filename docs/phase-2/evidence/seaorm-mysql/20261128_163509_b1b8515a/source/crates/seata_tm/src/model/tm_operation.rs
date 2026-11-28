/// 保留单次TM调用的原始意图，未知Commit不能被改写为Rollback。
/// 对应 Java: org.apache.seata.core.model.TransactionManager的方法集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmOperation {
    /// 创建全局事务。
    Begin,
    /// 提交原XID事务。
    Commit,
    /// 回滚原XID事务。
    Rollback,
    /// 查询原XID的TC状态。
    Status,
}
