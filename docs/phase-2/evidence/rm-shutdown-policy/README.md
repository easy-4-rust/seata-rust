# RM 停机边界验证

Rust 1.99.0 和 1.85.0 各执行 native_calls 七项与 native_callbacks 九项测试。

- 对端先完成真实 TCP 登记并收到原 BranchReport，扣留响应后关闭 RM。关闭在有界等待内完成，原调用返回携同一请求的 UnknownOutcome。原连接终止，客户端克隆发送前拒绝，监听器没有新的重连会话。
- 外部持有原分支日志租约时关闭 RM，返回日志未排空错误。原 socket 关闭。释放租约后可按原身份重新取得租约。
- 既有在途资源回调取消用例继续核对一次资源动作与持久 Pending。

[1.99 测试](1.99.0-green.log) · [1.85 测试](1.85.0-green.log) · [1.99 Clippy](1.99.0-clippy.log) · [1.85 Clippy](1.85.0-clippy.log)

削弱对照临时绕过 wait_idle，外部租约测试因关闭错误返回成功而失败。[失败日志](drain-bypass-red.log)。对照结束后原源码已逐字节恢复，双版本回归和严格 Clippy 均针对恢复后的正式源码运行。

用例使用正式 RM、传输和文件日志，TCP 对端为受控协议夹具。原 Java TC、真实数据库的停机故障，以及生产信号和应用流量排空继续验收。


## 原 Java TC 与 MySQL 的同进程 RM 重建

固定原 Java TC 与 MySQL 8.4.13 下，原 RM 先完成真实业务的本地提交与一次成功报告，随后消费 shutdown。独立 SQL 核对 004、013 摘要及 007 均保留，旧池采样确认关闭。同一进程以同一数据源适配器和现有 FileBranchStore 重建 RM，完成新会话登记与原 XID 的锁查询后，原 TM 才发起 GlobalCommit。

原始 Seata 帧核对两个不同 RM 连接各完成一次服务登记。原报告及成功响应位于旧连接，唯一提交回调及成功响应位于新连接，身份保持原 XID、BranchId、资源和分支类型。MySQL 原始报文核对业务 INSERT 及原业务 COMMIT 各一次，最终保留原业务行，Undo 与 TC 会话、分支和锁清空。

[Rust 1.99](../java-tc-changed-commit-report/run-graceful_rm_restart-1.99-a892ecd2/graceful-replacement-wire.json) · [Rust 1.85](../java-tc-changed-commit-report/run-graceful_rm_restart-1.85-d2b13d2d/graceful-replacement-wire.json)

场景 graceful_rm_restart 可通过原 run_desktop.ps1 复跑。两个工具链严格 Clippy 通过。证据回放拒绝把回调改到旧会话、缺失旧登记、缺失原业务 INSERT 和重复原业务 COMMIT。

本场景在一阶段报告完成后、全局结束前重建 RM，沿用当前进程已有的数据源和日志对象。进程间重建、运行中的数据库 COMMIT 停机及生产信号处理仍按各自验收项继续。首次运行曾因校验器参数更新不一致而中断，修复后以上两份完整现场记录通过。


## 原回调 COMMIT 确认滞留期间关闭 RM

新场景 callback_commit_shutdown 使用原 Java TC 与 MySQL 8.4.13。代理仅扣住二阶段终态事务的真实 COMMIT OK。独立 SQL 先核对同一 XID、BranchId 的提交终态、已删除 Undo、保留业务行和 32 字节摘要，然后才允许 Rust 关闭原 RM。

旧工作任务取消后，原日志保留 Pending。新 RM 在同一资源和日志上完成登记，原 TC 对同一分支重试。回调请求原字节保持一致，唯一成功响应来自新会话。新适配器读取已提交终态完成原 Pending，原 Undo 删除和终态写入各一次，业务和 TC 最终状态与普通提交一致。

[Rust 1.99](../java-tc-changed-commit-report/run-callback_commit_shutdown-1.99-be58b1cf/callback-shutdown-wire.json) · [Rust 1.85](../java-tc-changed-commit-report/run-callback_commit_shutdown-1.85-b5e81b15/callback-shutdown-wire.json)

两工具链严格 Clippy 通过。回放验证器拒绝旧会话成功响应、缺失原 OK 截留、重复 Undo 删除及重复终态写入。Rust 1.85 首次启动阶段的 Windows 监听进程查询超时，清理该轮夹具后重跑通过。原 COMMIT 尚在数据库执行时的关闭、Rollback 方向及生产信号处理继续验收。
