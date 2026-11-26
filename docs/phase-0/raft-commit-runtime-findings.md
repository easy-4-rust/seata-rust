# RAFT 混合提交恢复：运行时发现

来源为固定Java `cba73e180fd8896a99df6415b859d275122feaa7`、原JAR及独立前导/保留/reload观察候选。见[真实RAFT提交证据](evidence/local-go-java-raft-commit-timeout/README.md)。不得把参考源码风险、某个成功配置和完整生产验收混为一谈。

| 事项 | 状态 | 证据与范围 |
|---|---|---|
| 选定自然16 AT/TCC业务，3000ms选举窗口 | 通过本候选范围 | 自然超时、两次领导更替、真实确认丢回复、AT清Undo、TCC持久重放效果1、最终三个副本清理 |
| 1000ms下第二次选举多轮分票 | OPEN，实际观察 | 054700保留两个存活节点相同term各投自己的原行及SHA，60秒未形成稳定领导；终止并清理 |
| 连接已停止peer耗时与分票之间的原因 | 待验证推断 | 日志有约1000ms连接超时；不能仅由时间量级证明因果 |
| Prepare后更改TCC上下文的复制 | 静态来源风险，未活体验收 | 本例11==13，不能证明修改context后仍完整复制 |

原`AbstractCore.branchReport`先更新branchSession.applicationData，再调用changeBranchStatus；固定`RaftSessionManager.onBranchStatusChange`使用XID/branchId新建DTO后仅写status；`UpdateBranchSessionExecute`也仅更新status。该源码路径提示应用上下文更新可能未被复制。是否实际丢失，仍需让真实Prepare产生与11登记不同的13报告、观察原复制消息、领导切换与snapshot后原分支/回调，不能由静态分析直接声称已复现。

本轮原OwnedCommitAction不修改ActionContext。额外归档门禁明确绑定原11/12、原13/14与每副本分支的status/application_data，认证的是原本相同context在所选链路中的保留；测试拒绝把“副本彼此一致”冒充对原报告一致，也拒绝将改变后的登记/报告混入本选定范围。

```mermaid
flowchart LR
    R[原11登记上下文] --> P[真实Prepare]
    P --> B[原13报告与14确认]
    B --> L[原领导内存分支]
    L --> S[RAFT状态更新DTO]
    S --> F[副本与snapshot恢复]
    F --> C[实际二阶段回调]
    B -. status和application_data直接绑定 .-> F
```

JAVA-RAFT-RELOAD-001及metadata/logging原问题仍OPEN。3000ms成功不关闭1000ms分票现象，不代表全分区/并发/损坏/崩溃或生产就绪；静态context风险也不因不变context样例通过而关闭。正式Rust、完整WP00和其余Phase1–7继续按原计划推进。


## 后续真实上下文更新复现与候选验证（2026-11-13）

上文“静态风险/未活体验收”描述的是055235不变context样例的范围。随后按[独立计划](java-raft-context-replication-candidate-plan.md)完成[061711原实现复现](evidence/local-go-java-raft-context-reference/README.md)：Prepare在真实SQL提交后生成新token并通过原13/14报告，原领导持新值、副本持原11旧值；真实选主后回调缺值，真实业务拒绝确认，SQL确认量/效果0。这将JAVA-RAFT-BRANCH-CONTEXT-001升级为实际观察缺陷，原参考保持OPEN。

[064040候选证据](evidence/local-java-raft-context-candidate/README.md)以显式nullable更新标志携带状态与context，新follower仅TRUE应用值（含空/null），旧标志缺失/null/false保持原context。同字节Java25原类11失败/14控制→候选全通过，真实旧序列化blob兼容。统一补丁组三节点两次选主与快照/重启/丢成功回执后，更新context仍与原13及原SQL完全绑定，真实确认/持久重放效果1；112 Python和35类来源等证据详见主目录。

由于固定Jackson拒绝未知字段，候选仅认证全节点同补丁；新reader读旧消息不等于旧reader能读新消息，不声称混合滚升，也不重建历史丢失数据。其他原参考与1000ms分票问题不随本候选通过而关闭。
