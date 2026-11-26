# Java 指标生命周期候选与真实 FILE16 验证

主运行：[run_20261113_043739_9fbdad8c](run_20261113_043739_9fbdad8c/summary.json)，依据[指标候选计划](../../java-metrics-lifecycle-candidate-plan.md)。独立0004补丁只修改MetricsSubscriber，组合已认证前导/恢复候选；原源码/JAR、producer、事件及旧封存证据保持不变。

原GlobalTransactionEvent与MetricsPublisher约定doing事件的endTime为空。原subscriber重入16/17时却先扣active、增加失败再计算耗时，产生错误计数与空指针。候选在完成状态分派前识别未完成事件；Begin及三类Retrying仍正常统计。有效完成事件保留原名称/标签/计数；缺begin、倒序或溢出仅跳过Timer，不补零、不补当前时间。原始事件不被改写。

| 实际进程/阶段 | active | failed/二阶段超时 | failed Timer | AfterCommitted |
|---|---:|---|---|---|
| 三个TC各自启动、SDK启动前 | 0 | 0/0 | 0样本 | 0 |
| 原调度器自然进入16 | 0 | 1/1 | 1样本，5486ms | 0 |
| 第一次恢复、人工重入且TCC回复丢失 | 0 | 0/0 | 0样本 | 0 |
| 第二次恢复、原TCC重放成功 | 0 | 0/0 | 0样本 | counter/summary/timer各1，实际3ms |

8份真实registry快照与8份观察回执分别绑定进程和原事务。lost/final先等待同TC、同transactionId/状态16的精确DEBUG跳过标记，再请求采样；回执记录标记行SHA及原快照SHA，离线验证再次核对。原总线事件观察器确实收到null结束时间；最终真实AfterCommitted事件正常计数和计时，不能用“无异常”或全丢弃事件冒充通过。

只读控制入口使用MetricsManager的真实CompactRegistry，仅get/count/total；读取可创建空meter表项，但不添加样本、不调用measure/clear。Prometheus仅验证独立28094端口绑定，本轮未抓取Exporter数据，不将Exporter输出列为通过范围。

实际AT/TCC业务链路同时完整通过：一次Begin/Commit、零Rollback，自然16；3TC/3SDK、两次MySQL及FILE journal恢复；真实AT清Undo，真实TCC确认成功回复丢失后持久重放效果仍1，最终原TC会话/分支清理。独立SQL、控制台、5次原生查询30帧及全部原始SDK关联仍符合父验收。每轮19父覆盖类认证，5前导＋3恢复＋2指标/控制类共30条实际来源。

本轮Java77项执行：65目标使用实际CompactRegistry、12固定pin原回归。相同3个测试/Runner二进制对原JAR类为47失败/30控制，对候选77全绿。4源在重新编译前冻结，从快照重编后测试类及候选class均与先前字节相同；实际部署MetricsSubscriber与该测试字节一致。Java编译278个classpath项、1950文件前后核对。Python72项执行：父业务57重新运行，新增指标原始证据破坏15；异步标记门禁曾2失败/13控制，补齐后全绿。Go254/Rust117/Java617完整父范围明确继承，未重跑，不与77直接相加。

834仓库输入导入前冻结；40外部输入包含实际编译元数据、4源快照、类、参数、日志与D盘冻结脚本。原JAR/SDK/配置/Driver及类字节、源码、依赖结束复核后才写成功。自有进程/容器停止，28091–28095释放。首轮042504误把MySQL临时port0套接字当就绪，未启动TC；容器日志保留，后续改TCP就绪。042734实际链路成功但未显式绑定异步处理顺序，保留诊断，不替代本主运行。

JAVA-METRICS-RETRY-001：**原未打补丁参考仍OPEN；0004候选在本次FILE16生命周期范围通过**，本轮无原subscriber异常。不是全部恢复计数重建、去重、Exporter、真实FILE17/RAFT指标、全SQL/配置/并发/崩溃、正式Rust或完整WP00验收。分阶段冻结提案仍待确认，全部计划保持原范围。
