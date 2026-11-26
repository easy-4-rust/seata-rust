# FILE 自然16与真实 AT/TCC 部分提交、丢回复及持久重放

主运行：[run_20261113_035053_4ea827e1](run_20261113_035053_4ea827e1/summary.json)，依据[FILE混合提交计划](../../go-java-v1-file-commit-timeout-plan.md)。原 Java JAR 加已认证19类前导/保留候选，固定完整 Go SDK 真实注册并执行 AT/TCC 业务；原 Java/Go 参考、main/action/proxy 和既有封存证据保持不变。

| 阶段 | AT余额与Undo | TCC业务 | TC原会话/分支与锁 |
|---|---|---|---|
| 原调度器自然进入16 | 900，原身份Undo0及完整字节 | 预留10，确认0，效果0 | 原AT/TCC两分支，提交锁可用 |
| 首次TC/SDK/MySQL重启 | 原字节不变 | 同原预留 | 同原16/两分支，锁可用 |
| 人工重试，真实TCC成功回复丢失 | 900，真实AT清理Undo并答复 | 确认10，效果1 | 仍16，仅原TCC，锁可用 |
| 再次TC/SDK/MySQL重启 | 900，Undo空 | 确认10，效果1 | 同原16/TCC，锁可用 |
| 人工重放原TCC，真实回复送达 | 900，Undo空 | 确认仍10，效果仍1 | 原会话/分支移除，锁可用 |

原 TM Begin/Commit/Rollback 次数为1/1/0。代理阻断9个实际Commit请求，原调度器自然生成16；首次人工操作仅丢弃1个原TCC成功回复，AT真实成功回复正常送达。首次HTTP200/body500须匹配原TCC响应超时及原AT完成日志；最终HTTP200/body200还须独立SQL及TC结果，不仅凭HTTP状态。

MySQL只有accounts、undo_log、commit_account、commit_reservation；TC实际session/lock使用同步FILE。3个独立TC、3个独立SDK及两次真实MySQL重启。两次启动输入逐文件分别与上一轮停机journal原字节相同；journal依次1542→1736→2313字节，为首次人工操作移除AT及最终清理正常追加，未要求全过程字节不变。最终D盘日志与停机快照一致。

原AT report.status2、TCC report.status1，以及可选应用上下文，均从真实13号BranchReport帧严格解析。两类报告分别匹配同连接/请求ID的成功14回复，必须先于对应首次Commit回调。保留分支的状态和完整应用上下文、实际Commit回调上下文均与原报告一致；空字段明确记录None。TCC业务幂等来自本次业务本地事务、原身份持久表及行锁，不能声称SDK自动保障任意业务。

五阶段独立SQL、控制台、5次只读原生查询的30原始帧分别核对。观察器只发101/15/21，不注册RM或创建/结束事务。SDK帧另验证原Begin/Commit目标XID、注册、方向/外层类型、RPC配对、两分支资源/回调、真实丢包及人工操作顺序。每轮5个前导主类＋3个保留主类共24条实际来源，从原stdout重新解析；19类认证链与原JAR/SDK binary/配置/JDBC classpath文件逐轮前后相同。

本轮新执行Python57项：13个既有归一化混合控制、8个FILE控制、36个真实证据破坏测试。FILE门禁曾7失败/1控制；AT恢复状态、原报告和上下文曾3失败/14控制；报告回复缺失/晚到曾2失败/34控制，补齐后57全绿。033951、034753两次实际链路成功但门禁较弱，保留为诊断，不替代主运行。归档器首次遇到tuple/JSON数组表示差异及继承EXE文件名差异，修正了离线归档适配；未修改业务证据或运行源码。

819项输入在动态导入前冻结，包含已认证父783输入、FILE共享辅助代码与独立候选类。Go原生产/候选cohort、原JAR、SDK binary、配置及类字节结束时再次核对，完整性检查在写成功前完成。自有TC/SDK/代理停止、容器退出，28091/28092/28093释放。Go254、Rust117、Java617明确继承，本轮未重跑，不与57直接相加。

原指标订阅者的GlobalTransactionEvent.getEndTime为空异常本轮出现2次，见[事件记录](run_20261113_035053_4ea827e1/known-metrics-events.json)，JAVA-METRICS-RETRY-001仍OPEN。业务恢复通过不代表该指标问题已修复。

范围仅此候选的FILE自然16混合链路；不是完整TCC Fence、RAFT真实业务、额外配置、所有SQL/并发/崩溃矩阵或正式Rust验收。原版FILE/RAFT缺陷、元数据/日志、指标、混合资源标识、生产认证、完整WP00及Phase1–7仍开放；分阶段冻结提案保持待确认。
