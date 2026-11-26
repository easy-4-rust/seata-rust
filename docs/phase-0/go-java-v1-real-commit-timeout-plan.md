# 真实 AT＋TCC 提交重试超时与持久重放

本增量延续 WP00、REC-TIMEOUT-008 和已确认ADR-0008，不修改门禁，不启动正式Rust。父证据为真实AT自然17及持久回滚重放；纯AT的AsyncCommitting不作为自然16证据。

1. 在独占D盘MySQL中使用原TC DB schema、真实AT业务/Undo及一个真实TCC预留/确认业务。完整原Go SDK注册两类资源，TCC先注册分支，AT后执行SQL；独立SQL证明两原BranchId、原AT Undo完整字节、真实TCC预留已提交。
2. 原TM只发一次Begin和Commit。代理丢弃真实TC二阶段Commit指令，不伪造RM成功响应。原TC调度器在测试专用有限期限后自然写入CommitRetryTimeout16，保留原AT/TCC分支；提交已释放全局锁，不能要求复制回滚17的锁状态。
3. TC/SDK进程及数据库重启后，以原XID、两分支、AT Undo、TCC预留和控制台证明恢复。不得手动UPDATE TC状态、合成Undo或补写业务结果。
4. 实际人工重试：原TCC RM执行真实本地确认，代理丢弃其实际成功响应；原AT RM实际清理Undo并正常答复。分别核对TC仍为16、仅TCC分支保留、AT余额保持900、Undo已清理、TCC确认10且业务效果计数1。
5. 再次重启TC/SDK/MySQL，重复人工重试原TCC分支。TCC业务以原XID/BranchId持久表及行锁实现幂等，重复确认不再次增加业务计数；最终原TC会话/分支/锁为空，AT业务仍900/Undo为空，TCC确认10、效果计数1。
6. 每阶段归档原帧及其真实转发/丢弃归属、实际资源注册/请求ID、独立SQL、控制台、调度器自然16日志、TC/SDK进程及MySQL启动身份、输入/依赖/二进制和实际加载类。验证器负向测试必须拒绝伪16、错误分支类型/资源/请求ID、跨阶段假丢包、缺少原AT清理或TCC重复业务效果。

这是选定混合事务的提交恢复证据，TCC业务幂等由本次真实业务实现负责；不能声称原Go SDK自动为任意业务提供幂等，也不等于完整TCC Fence/空回滚/悬挂、全SQL崩溃矩阵、FILE/RAFT安全恢复、混合语言共享资源或生产Rust交付。008、WP00及完整计划只按原范围更新，不能以一条链路直接全部关闭。
