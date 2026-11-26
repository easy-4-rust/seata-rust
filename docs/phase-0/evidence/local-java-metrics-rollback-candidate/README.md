# Java 指标候选与真实 FILE17 回滚恢复

主运行：[run_20261113_045658_cc75268a](run_20261113_045658_cc75268a/summary.json)，依据[回滚验证计划](../../java-metrics-rollback-candidate-plan.md)。本轮复用独立0004的同一生产MetricsSubscriber字节，新增只读回滚观察入口；原Java源码/JAR、producer、Go业务及既有封存证据保持不变。

| 实际阶段 | active | failed/二阶段超时 | failed Timer | AfterRollbacked |
|---|---:|---|---|---|
| 三个TC各自启动、SDK启动前 | 0 | 0/0 | 0样本 | 0 |
| 原调度器自然进入17 | 0 | 1/1 | 1样本，5485ms | 0 |
| 第一次恢复、补偿成功回复丢失 | 0 | 0/0 | 0样本 | 0 |
| 第二次恢复、原分支重放成功 | 0 | 0/0 | 0样本 | counter/summary/timer各1，实际3ms |

真实完整Go SDK只请求一次Begin/Rollback。代理阻断原回滚请求，由原TC调度器自然超时；实际SQL余额900及完整Undo、原分支和锁保留。两次TC/SDK/业务MySQL恢复使用前次停止后的完全相同FILE日志。第一次人工重试真实补偿使余额1000、Undo空，成功响应被丢弃而TC仍保留17/分支/锁；第二次人工重放同一分支，余额保持1000，仅防悬挂Undo记录为1/{}，最终TC会话/分支清理。

8份实际非重置Registry快照、8份观察回执分别绑定实际TC进程、原transactionId/名称/标签/时间/标志。两个恢复进程确实收到原endTime为空的RollbackRetryTimeout doing事件；先等待同TC候选的精确DEBUG处理标记，再请求采样，回执保存原行和快照SHA。最终原Rollbacked/retryGlobal=true及AfterRollbacked事件均实际出现，不能通过丢弃所有完成事件通过验收。实际get/count/total不调用measure/clear，不修改producer、事件或指标。Prometheus只验证28094绑定，未抓取，不据此验收Exporter。

本轮新执行Python71项：原FILE17业务44重跑，回滚阶段样例12及真实指标副本破坏15。阶段样例的旧提交校验器RED为3失败/9控制，通过回滚实现后全绿；样例明确仅为单元输入，正式运行从实际事件/Registry及回执重新推导，不合成运行证据。已认证Java77项（65目标、12固定pin原回归）继承，未本轮重跑：本轮重新编译MetricsSubscriber与该已测试class逐字节一致。Go254/Rust117/Java617完整父范围同样继承，不重复计入新执行数。

854仓库输入和40外部输入冻结并结束核对，认证FILE17与指标父目录；278classpath项/1950依赖文件前后一致。20生产覆盖class＋1只读观察class，三个TC的5前导＋3恢复＋2指标/观察主类共30条实际来源分别重算。原Go可执行字节与已封存FILE17相同。独立SQL/控制台/5原生查询30帧、原RPC关联、两次journal恢复输入及最终磁盘日志完整重验。

自有3TC/3SDK及容器均停止，28091–28095释放，原参考/JAR及候选字节未变。JAVA-METRICS-RETRY-001原参考保持OPEN；0004分别获得选定FILE自然16和自然17的生命周期证据，不代表两者并发、全重启指标重建/去重、Exporter、RAFT/所有SQL/配置/崩溃矩阵、正式Rust或完整WP00验收。分阶段冻结提案仍待确认，全部计划保持原范围。
