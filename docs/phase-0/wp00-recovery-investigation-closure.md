# WP-00 恢复调查逐项核对

本次核对对应现行清单中的原任务：**“Run Java retry-timeout recovery investigation REC-TIMEOUT-001–010 or explicitly document blocked cases.”** 调查门禁已完成：十项均有实际调查及范围记录，未满足的恢复要求明确保留受阻状态。它不是十项安全恢复验收通过，不关闭 WP-00，不修改 WP-01 对 WP-00 的依赖，也不使待确认的分阶段冻结提案生效。

规格事实源为[恢复调查的原十项要求](java-tc-retry-timeout-recovery.md#5-required-java-regression-tests)及[现行验收清单](v1-implementation-acceptance-checklist.md)。下表同时列出原实现事实、后续候选和仍需完成的验收，避免用候选结果覆盖原缺陷。

| ID | 已调查的实际结果 | 调查结论与仍受阻的恢复要求 |
|---|---|---|
| 001 FILE / 16 | [原完整 TC](evidence/local-file-retry-timeout/README.md)保留磁盘会话/分支，但启动退出 -1，无法提供查询；[0002 候选](evidence/local-java-retry-timeout-candidate/README.md)另取得保留及所选解锁策略证据 | 调查完成；原版安全启动要求被实测反证。修复发布、完整配置/崩溃矩阵及最终运维处置仍未验收，不能标成原版通过 |
| 002 FILE / 17 | 原 reader 过滤为 0 会话/0 分支，完整 TC 查询 Finished15、原锁不再持有；候选另保留原会话 | 调查完成；原版保留/删除意图与安全处置仍未关闭。Finished 不证明补偿成功，候选不修复历史已丢数据 |
| 003 RAFT / 16 | [原三节点实测](evidence/local-raft-retry-timeout/README.md)经两次选主及原盘重载仍可查询原 16/分支/锁；两次实际异步 reload 异常；原版人工重试及最终恢复未执行 | 调查完成；原参考安全 reload 受 JAVA-RAFT-RELOAD-001 阻断。[候选自然提交恢复](evidence/local-go-java-raft-commit-timeout/README.md)不能替代原版无异常验收或全分区/崩溃证明 |
| 004 RAFT / 17 | 原集群保留原 17/分支/锁，并有两次 reload 异常；原版人工重试及最终恢复未执行；[候选自然回滚](evidence/local-go-java-raft-at-timeout/README.md)补齐选定真实补偿链路 | 调查完成；原版 reload 仍失败。候选所选配置通过不扩大为全部选举超时、分区、解锁或生产就绪 |
| 005 DB / 16 | [原 DB 存储重启](evidence/local-backend-retry-timeout/README.md)可查询原 16 并接受人工提交；另有真实 AT＋TCC 自然 16、两次持久重启与原分支重放 | 选定调查范围完成；早期种子/模拟 RM 与后续真实业务分别记录。不等于正式 Rust、所有 SQL/故障组合通过 |
| 006 DB / 17 | 原 DB 重启保留原 17/分支/锁并接受人工回滚；[真实 MySQL AT](evidence/local-go-java-real-at-timeout/README.md)补偿及丢回复后重放不重复扣/加余额 | 选定调查范围完成；完整生产 RM 的原子性、崩溃窗口、并发 SQL 仍在后续验收 |
| 007 Redis / 16、17 | 八个 DB/Redis 种子组合中的四个 Redis 用例覆盖两状态、零/一个分支，原 TC 重启、查询及人工重试均有原始记录 | 选定调查范围完成；使用模拟 RM，Redis 下自然超时与真实业务补偿/持久幂等未由此证明，仍需后续补齐 |
| 008 重启后人工提交 | [DB 真实 AT＋TCC](evidence/local-go-java-real-commit-timeout/README.md)原 TM 只发一次 Begin/Commit；自然 16，重启后 AT 清 Undo，TCC 实际成功回复丢失，再次重启/同分支重放后确认量仍 10、效果仍 1；另有 FILE/RAFT 候选链路 | 选定真实业务调查完成；幂等来自该业务持久事务，不声称通用 TCC Fence 或任意业务自动安全。RAFT Prepare 上下文修复仅认证[统一补丁组](evidence/local-java-raft-context-candidate/README.md) |
| 009 重启后人工回滚 | DB 自然 17；实际补偿余额 900→1000，丢成功回复后 TC 仍持有原分支/锁；再次 TC/SDK/DB 重启后重放仍 1000，最终原会话/锁清理；另有 FILE/RAFT 候选 | 选定真实补偿调查完成；最终同身份防悬挂 Undo=1/{} 被明确保留，不误称 Undo 绝对空。所有 SQL/并发/崩溃窗口仍在后续验收 |
| 010 无剩余分支 | DB/Redis × 16/17 四个零分支用例的原人工重试成功、会话删除，重复命令被原 TC 精确拒绝且无复活/额外资源动作 | 选定调查范围完成；重复返回 HTTP500 的明确拒绝，不冒充成功 no-op。不覆盖所有存储/并发/断电竞态 |

原要求中的重启前状态、重载会话/分支/锁、控制台、重试结果、最终状态与存储日志，已执行部分保留原证据，未执行部分明确记为受阻或未覆盖。001 启动失败导致无法取得查询/重试；003/004 原版人工重试及最终恢复未执行，原版安全恢复验收受 reload 异常阻断，候选另列。002 Finished/锁释放是失败事实，不能填成业务成功；003/004 的异步异常也不由可查询性覆盖。

本轮只读重核十一组历史清单及其列出的证据文件，并重新核对原 FILE/RAFT 失败、八个 DB/Redis 组合、两条 DB 真实补偿/提交重放的关键状态、余额/效果及回复丢失结果。没有启动 TC/数据库，没有新执行 Java/Go/Rust 业务或单元测试；历史测试数量不计作本轮新测试。旧清单、来源快照和运行均未补写或重新归档。

[本轮实际审计](evidence/local-wp00-recovery-investigation-audit/README.md)重新匹配十一份历史清单及 20,880 个列出文件的 SHA-256；原文件内容和清单都未改写。它证明本次核对所引用的历史资料完整及选定关键事实一致，不等同于本轮重新执行全部旧测试或重建当年的工具/服务。

尚未关闭的工作继续由原运行时门禁维护：原 FILE/RAFT 缺陷及候选完整验收、Redis 真实资源链路、指标/Exporter、完整故障/配置/SQL 矩阵、生产认证、Go 状态和空报告语义、正式 Rust 组件。共享锁问题仍按用户要求延期；本次不恢复其实现、测试或封存。
