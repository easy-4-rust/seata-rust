# 真实 RAFT 自然16：AT/TCC 提交与持久重放

主运行：[run_20261113_055235_cb3164e4](run_20261113_055235_cb3164e4/summary.json)，依据[RAFT提交计划](../../go-java-v1-raft-commit-timeout-plan.md)。本轮明确使用3000ms选举窗口；1000ms失败配置仍OPEN，不以本次成功覆盖。原Java JAR、Go混合业务/OwnedCommitAction及20生产覆盖class未修改，三个只读控制/诊断class也与认证RAFT17父字节一致。

| 阶段 | AT余额/Undo | TCC持久业务 | TC与副本 |
|---|---|---|---|
| 原调度自然进入16 | 900/完整原Undo | 预留10，确认0，效果0 | 原AT/TCC两分支，提交锁可用 |
| 第一轮领导切换与DB/SDK重启 | 900/原Undo完全相同 | 原预留10保持 | 新任期成功reload，两分支保持 |
| 人工提交，丢TCC原成功4回复 | 900/Undo清空 | 确认10，效果1 | 仍16，仅原TCC保留 |
| 原节点snapshot恢复、第二次切换 | 900/Undo空 | 确认10，效果1 | 原TCC身份、状态、上下文保持 |
| 再次同分支重放并恢复全副本 | 900/Undo空 | 确认仍10，效果仍1 | 三副本会话/分支/锁均清理 |

原完整SDK仅一次Begin/Commit、零Rollback。代理只阻断原Commit请求或丢一个实际TCC成功回复，没有改状态、合成成功或用纯AT AsyncCommitting代替自然16。原AT本地SQL/Undo与TCC预留、确认均真实执行；MySQL只有四个业务/Undo/TCC表，TC实际session/lock为RAFT。

5个TC JVM、3个Go SDK进程、两次业务DB重启；捕获不同领导的任期1→2→3。所有副本核对实际Seata/JRaft任期/peer/领导标志、当前任期成功reload、applied追上committed及重复领导确认。SDK路由、人工操作和五阶段SQL/控制台/原生查询均有前后同PID/peer/term采样。只允许已完成的原AT从partial集合退出，不能丢原TCC或更改其状态/上下文。

六次真实snapshot、两个原盘恢复、真实protobuf index/term/文件SHA与两份JFR重读分别核对；恢复日志精确index绑定先前节点代次/配置/原文件。五代日志与PID全集检查，停止回执的身份、before样本和原进程记录直接绑定切换前确认领导。本轮将此前额外停止身份门禁直接纳入运行epoch。关键类来源20条、五次只读原生查询30帧、原13报告/成功14确认、注册/回调3/4、目标XID/BranchId、方向/外层类型/相关ID及操作顺序完整复核。

运行87 Python：13归一化混合业务、33原始业务破坏、25集群/partial、3元数据输出、9完整证据副本、4停止身份。旧回滚校验不能接受混合16的行为RED为1失败/20控制；提交/partial实现后25全绿。首轮054436业务完整执行，但一个迁移测试残留导入路径错误，86/87通过；修正后对该真实数据87通过，正式运行另起新输入epoch。

最终另有4项上下文绑定验证，先RED3失败/1控制，再通过，合计新执行91项。该[额外认证](run_20261113_055235_cb3164e4/context-binding-certification.json)从原11/12提取登记context与branchId，并把原13/14已确认报告直接绑定17条各副本分支观测；“所有副本同改成错误context”也必须拒绝。代码、测试和归档驱动器3个输入独立冻结；最终认证必须同时通过运行和此额外门禁，不能只看runtime succeeded。

本例原11与13的AT空context、TCC真实JSON context分别相同（UTF8字节关系及SHA已归档）。OwnedCommitAction.Prepare不修改ActionContext，因此**本轮不认证Prepare后新增/改写上下文的RAFT复制**。固定Java的branch report更新仅复制status存在静态来源风险，见[运行时发现](../../raft-commit-runtime-findings.md)，尚非本例活体复现，也不能用本例通过关闭该能力。

948运行输入、341classpath/2821依赖、原JAR/SDK/Driver/配置与覆盖字节前后一致；Go可执行与认证FILE16父字节完全相同。Go254/Rust117/Java RAFT52继承未重跑，不计入91。自有进程/容器停止，11端口释放；未创建/切换分支。原metadata/logging/reload参考问题仍OPEN，未验收RAFT指标、Exporter、通用TCC Fence、全SQL/分区/并发/损坏/崩溃、生产认证、正式Rust或完整WP00。

1000ms的054700运行在第二次领导停止后，两个存活节点多轮相同任期各投自己，60秒未出现满足门禁的稳定领导，最终失败并清理。原配置、停止回执、每条投票原行/SHA及共同term已归档在[失败配置记录](election-1000ms-failure.json)。连接已停止peer超时约1000ms是日志事实；其与选举窗口同量级导致分票只是推断。后续3000ms是独立配置证据，**不关闭1000ms不稳定问题**。
