/// 恢复管理器内部实际控制事务的固定动作；不代表业务 SQL 次数。
/// 对应 Java: 无直接对象；WP07 MySQL 恢复控制事务观测扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MySqlRecoveryOperation {
    /// 持久登记原分支身份。
    Enroll,
    /// 持久观察正式 TC 回调决定。
    Observe,
    /// 绑定提交方向或读取原提交方向。
    BindCommit,
    /// 绑定回滚方向或读取原回滚方向。
    BindRollback,
    /// 提交清理或读取既有提交终态。
    Commit,
    /// 补偿回滚或读取既有回滚终态。
    Rollback,
}
