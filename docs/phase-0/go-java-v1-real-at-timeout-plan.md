# 真实 AT 重试超时与持久重放验证

本增量延续 WP00、ADR-0008 和 REC-TIMEOUT-009，不改变冻结依赖或验收标准。使用既有0001–0006 Go候选、固定Java TC JAR及已认证的独立前导分片覆盖类。生产Rust仍未开始。

1. 独占D盘MySQL数据目录与loopback端口，原TC DB schema和真实业务/Undo schema共用本次私有库。实际完整Go SDK创建一个全局事务、真实SQL分支和Undo，独立SQL绑定原XID/BranchId及完整Undo哈希。
2. 原TM只结束一次；代理仅截断真实TC回滚指令，不伪造分支或成功响应。以短的测试专用重试期限，让原TC调度器自然持久化RollbackRetryTimeout17。不得UPDATE/插入TC状态制造自然超时。
3. 保存控制台、global/branch/lock表及业务/Undo快照；停止拥有的TC/RM，重启原MySQL，重新启动TC/RM。原XID、分支、资源、锁和Undo应保留；17不应自行重放。
4. 实际控制台人工重试原事务；实际RM补偿到原余额并删除原Undo。代理丢弃实际成功回滚响应，证明TC仍保留失败事务与分支，而业务已恢复。
5. 再停止拥有的TC/RM并重启数据库；新RM读取持久结果。第二次实际人工重试原分支应安全完成，业务不再变化；按固定Go语义允许且核对同XID/BranchId的status1防悬挂Undo。不得以空Undo表作为唯一成功标准。
6. 每阶段归档完整原帧、连接/请求ID、实际注册、响应去向、SQL结果、进程与数据库启动身份、输入/产物哈希、实际加载类来源和清理。验证器必须拒绝空快照、错XID/分支、伪17、错误回调关联或未实际丢弃的响应。

CommitRetryTimeout16另行处理：固定Java纯AT提交跳过同步分支并进入AsyncCommitting，异步处理方法不检查maxCommitRetryTimeout。不能把强行改成16或有界观察未出现16当作自然16证明。本增量不关闭REC-TIMEOUT-008、FILE/RAFT缺陷、全SQL原子性矩阵或WP00。
