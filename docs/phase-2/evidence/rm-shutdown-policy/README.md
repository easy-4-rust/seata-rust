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


## 回滚回调 COMMIT 确认滞留期间关闭 RM

场景 callback_rollback_shutdown 在同一原 Java TC/MySQL 配置上扣留回滚事务的真实 COMMIT OK。独立 SQL 核对 phase=2、terminal=1、业务行已补偿删除、一个 Undo 防悬挂记录和原 32 字节摘要后才允许关闭。Rust 在等待原 GlobalRollback 响应的同时按该标记关闭旧 RM，原日志保持 Rollback Pending。

替换 RM 重新登记后，原 TC 在新会话上重试同一原回调，返回状态 8。原始 MySQL 参数核对一次业务补偿 DELETE、一次 Normal Undo 删除、一次防悬挂 INSERT 和一次回滚终态更新，这些动作位于同一原连接和 COMMIT。替换会话只观察已持久终态，日志状态为 Pending、Completed，TC 会话、分支和锁最终为零。

[Rust 1.99](../java-tc-changed-commit-report/run-callback_rollback_shutdown-1.99-e7dc02c0/verified.json) · [Rust 1.85](../java-tc-changed-commit-report/run-callback_rollback_shutdown-1.85-7224554d/verified.json) · [反例回放](../java-tc-changed-commit-report/run-callback_rollback_shutdown-1.99-e7dc02c0/shutdown-replay-controls.json)

原 TM 回滚请求可能先到本地期限。用例保留原 UnknownOutcome，并只查询原 XID 等待 TC 完成，始终仅一个 GlobalRollback。首次现场曾要求该请求必须获得成功响应，实际暴露了此未知窗口，修正用例的结果分类后双版本通过。验证器拒绝缺失补偿、重复围栏、旧会话成功回包和伪造原 OK 已交付的报文。

本轮覆盖已持久化且响应滞留的窗口。数据库仍在执行 COMMIT 时的关闭及生产信号处理继续验收。


## 原提交回调 COMMIT 执行期间关闭 RM

场景 callback_commit_in_flight_shutdown 在自有 MySQL 容器临时启用一秒 binlog 组提交等待。代理向原 MySQL 转发回调事务的 COMMIT，独立 SQL 将报文握手中的物理连接 ID 与 performance_schema 中的 Query、COMMIT、waiting for handler commit 以及原 XID/BranchId 绑定。此时业务行与 Normal Undo 仍在，提交终态仍为零。

第一次 SQL 快照通过后才允许 Rust 消费原 RM 的 shutdown。旧池确认关闭后，Rust 发布关闭观察标记。第二次独立 SQL 仍查到同一个 MySQL COMMIT 正在执行，数据库可见状态与第一次完全相同。运行器记录快照后恢复该自有容器的组提交设置，再允许新 RM 登记与读取最终持久状态。

原 TC 的固定回调等待结束后，在新会话重发原分支请求。日志由 Pending 转为 Completed，原 Undo 删除和终态写入各一次，最终业务保留、TC 会话/分支/锁清空。原 COMMIT 的成功响应未交付给旧 RM。

[Rust 1.99](../java-tc-changed-commit-report/run-callback_commit_in_flight_shutdown-1.99-e705f1b6/verified.json) · [Rust 1.85](../java-tc-changed-commit-report/run-callback_commit_in_flight_shutdown-1.85-f7490596/verified.json) · [错误快照与重复动作控制](../java-tc-changed-commit-report/run-callback_commit_in_flight_shutdown-1.99-e705f1b6/shutdown-replay-controls.json)

两套工具链严格 Clippy 通过。首轮用例观察期限未覆盖固定 TC 的 15 秒回调等待，另一轮汇总缺少新场景映射，修复后以上完整记录通过。Windows CIM 监听查询曾反复超时，运行器现在回退 netstat 并继续核对真实监听 PID，仍拒绝其他进程占用目标端口。

本轮覆盖 Commit 方向。Rollback 的 COMMIT 执行中关闭、生产信号及应用业务排空继续验收。
