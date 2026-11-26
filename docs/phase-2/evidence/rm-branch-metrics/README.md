# WP07 RM 日志与分派指标

事实源：[WP07 可观测性契约](../../wp07-observability-contract.md)与[V1 验收清单](../../../phase-0/v1-implementation-acceptance-checklist.md)。本增量不关闭完整 WP07 或改变原回调、日志格式、Undo 格式和事务决策权。

## 状态与事件

RmMetricsSampler 只含固定原子计数，不持有文件、租约或目录。FileBranchStore 与 ResourceManager 签发同一日志打开周期的只读源；手工分派和正式回调共享统计。状态在原追加成功同步、索引更新之后发布，重开时按通过校验的日志重建。保留旧采样器不阻止原路径重新取得文件锁。快照逐字段读取，counter 饱和不回绕。

| 指标 | 含义与固定维度 |
|---|---|
| seata_rm_journal_branches | 两种 phase、四种保留 state 的 gauge；不是 TC 活跃分支数量。 |
| seata_rm_dispatch_active | 当前已轮询分派，包括等待同一分支租约的作用域。 |
| seata_rm_dispatch_total | Applied、Rejected、RetrySafe、Unknown、ManualIntervention、两种终态复用、错误和取消九种 outcome。 |
| seata_rm_resource_attempts_total | 两种 phase 下 apply/recover 的入口尝试，不作为 SQL 生效次数。 |
| seata_rm_journal_faulted | 追加写入或同步失败；gauge 只保留最后确认值。 |
| seata_rm_journal_closed | 日志所有者进入销毁；不等于操作系统文件锁释放证明。 |

共 2×(4+1+9+2)+2=34 条固定 RM 序列。Completed 条目可历史保留，不能解释为活跃业务数或数据库原子性证明。分派取消后后台磁盘任务仍可更新 gauge；取消只计 cancelled。重开会重建 gauge 并重置分派累计，重建前已制造非零 duplicate_completed 后验证归零。

## 组件与 HTTP 证据

[缺采样 API 的 RED](red.log)、[缺导出入口的 RED](export-red.log)。资源测试覆盖两种方向的所有五种资源结果、终态复用、恢复不重放、方向冲突和错误保留 Pending。两个同分支等待者在一次资源调用期间 active=2，取消后 cancelled=2、Pending=1。真实文件测试验证重开、旧采样器不保留文件锁，实际只读 OS 文件句柄制造写失败后 faulted=1、Pending=1、Completed=0。

[把发布提前到写入之前的变异](pre-sync-mutation-red.log)实际得到错误的 (Pending,Completed)=(0,1)，目标断言失败；[还原源码后](restored-write-green.log)通过。并行上限测试验证累计饱和而 active 仍归零。

[原连接回调与认证 HTTP](export-green.log)验证实际 Signed ID、首次提交、重复提交及反向冲突；资源仅一次执行。采集到进行中的 active/Pending，完成后精确检查 34 行，错误令牌为 401，无 XID、BranchId、资源、application_data、路径和凭据哨兵。RM 关闭后外部仍持有日志时 closed=0，日志最后所有者退出后仍能采集最终值且 closed=1。

## 四组完整原 Java TC/MySQL 现场

原物理 COMMIT 在 RM 关闭前后仍在同一 MySQL 会话执行，旧资源 Future 被取消而不假设数据库已回滚。旧会话分派保留 Pending；新会话收到同一原分支后只调用 recover，读取权威终态，再同步 Completed。共享日志累计没有因 RM 所有者重建而重置；apply/recover 各一次，取消数与旧连接实际回调数量对应。

- [1.99 Commit](../java-tc-changed-commit-report/run-callback_commit_in_flight_shutdown-1.99-fbbc182b/rm-branch-metrics.json) / [完整现场](../java-tc-changed-commit-report/run-callback_commit_in_flight_shutdown-1.99-fbbc182b/verified.json)
- [1.99 Rollback](../java-tc-changed-commit-report/run-rollback_commit_in_flight_shutdown-1.99-a5e6630d/rm-branch-metrics.json) / [完整现场](../java-tc-changed-commit-report/run-rollback_commit_in_flight_shutdown-1.99-a5e6630d/verified.json)
- [1.85 Commit](../java-tc-changed-commit-report/run-callback_commit_in_flight_shutdown-1.85-1439e6c2/rm-branch-metrics.json) / [完整现场](../java-tc-changed-commit-report/run-callback_commit_in_flight_shutdown-1.85-1439e6c2/verified.json)
- [1.85 Rollback](../java-tc-changed-commit-report/run-rollback_commit_in_flight_shutdown-1.85-f403c4ab/rm-branch-metrics.json) / [完整现场](../java-tc-changed-commit-report/run-rollback_commit_in_flight_shutdown-1.85-f403c4ab/verified.json)

各目录保留 rm-metrics-native-binding.json 与九种 rm-metrics-controls.json：错误的新执行计数、取消伪装成完成、丢失 Pending、RM 重建时错误归零、身份标签、布尔伪计数、错误原帧数量、改写旧回调正文及移动旧回调到新连接均被拒绝。完整原生/MySQL 线缆断言按最终共享检查器再次重放，输出到新的派生文件以保留原报告：[四组重放](final-wire-replay.json)。

现场由 run_desktop.ps1 的 callback_commit_in_flight_shutdown / rollback_commit_in_flight_shutdown 场景执行，工具链参数 1.99 / 1.85。未修改原 TC 或原 TM 的超时。进程和自有容器均已清理。

## 自然重叠与迟到 Failed 的诊断记录

1.85 回滚初轮的 [旧验收脚本失败](java-rollback_commit_in_flight_shutdown-1.85.log)来自写死“一个旧回调”：原始帧实际有两个同身份旧请求与一个新会话请求，计数 active=2、cancelled=2、apply=1、recover=1。现在要求旧请求完整正文一致、关联唯一、新会话只回复一次，取消数精确等于旧原始帧数；首次快照 active 只作为 1..旧最终请求数的并发观测。资源 SQL 仍必须仅执行一次，不能用额外回调解释重复资源效果。

这个初轮未执行外部最终数据库计数，未归入上面的四次完整通过。保留原输入后，[计数与原帧绑定重放](native-binding-replay.json)通过；[完整原生/MySQL 线缆重放](overlapping-wire-replay/replay-verified.json)也验证一次补偿、一次 Undo 删除、一次围栏、一次终态写入及同一未决物理 COMMIT 的前后快照。[原始资料](overlapping-wire-replay/observation.json)与原始帧/SQL 文件保留在同目录，属于已有资料重放而非新现场运行。

原 TC 的 Failed/RollbackRetrying 回包在原请求之后 15039.3333ms 到达，原 TM 的 RPC 期限为 10 秒，实际调用只保留 Unknown。验收只接受原请求关联、明确迟到、消息内原 XID/BranchId、RollbackRetrying，以及后续只读 Finished；[四个反例](overlapping-wire-replay/late-failed-reply-controls.json)拒绝提前 Failed、错误状态、异身份和把已观察结果说成未知。客户端原期限与重试策略保持原行为。

## 回归与边界

Rust 1.99/1.85 各 [697 项全工作区全功能通过](1.99-workspace.log)、[313 项外部环境 ignored](1.85-workspace.log)。四组原 TC 是另行实际执行的通过，不能替代其他 ignored。最后加强“已有非零 counter 在重开归零”的断言后，[1.99 journal](1.99-journal-final.log)、[1.85 journal](1.85-journal-final.log)另行通过。[1.99 严格 Clippy](1.99-clippy.log)、[1.85 严格 Clippy](1.85-clippy.log)覆盖 RM、observability、MySQL 全目标全功能。[格式](format.log)与[证据核对](validation.log)单独记录。

完整 RM 出站结果、MySQL 本地状态、重试、生产指标采集/权限/TLS/告警和 WP05–07 门槛仍开放。原 Java TC 场景核对只读采样器、原帧和数据库事实，HTTP 在受控原连接回调夹具中验证；未把两者组合说成生产采集已完成。
