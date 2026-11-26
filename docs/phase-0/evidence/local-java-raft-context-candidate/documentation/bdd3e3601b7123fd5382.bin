# RAFT TCC 上下文更新：真实复现与独立修复候选

延续ADR-0008、REC-TIMEOUT-008及RAFT混合提交发现。Brownfield增量，以现有Markdown/ADR为规格事实源；不改正式WP依赖、原参考或已封存输入，不初始化工具。

真实业务fixture在原TCC Prepare提交数据库预留后，才向BusinessActionContext新增prepared-units、prepared-branch、prepared-token。预留表持久保存同一单位数/token。原SDK负责11注册、13更新报告及14确认；Commit必须从TC原回调读取这些新值，并在SQL事务中与原预留行比对，再执行确认。缺值不得从数据库补造或成功返回。原Go SDK及兼容候选不改。

先用现有20生产class候选复现：原11不含新值、原13/14确有新值；原领导内存与原报告一致，副本和快照若仍旧值须明确记录。真实选举后人工提交应正向观察原回调缺少新值导致业务拒绝、确认效果未发生，而不是只看到字段差异。诊断拓扑检查可以在独立副本中排除待测context一致性，以收集错误后果，但不得把这条诊断路径标成恢复通过。

若复现成立，新增独立Java0005候选，保持V1客户端报文不变：

- 状态复制DTO同时携带当前applicationData与显式nullable更新标志。
- 新follower仅在更新标志为true时应用context；支持非空、空串和明确null清除。
- 缺失标志/null标志/false表示历史status-only日志，保留已有context，避免把旧消息默认null当作清除指令。
- DTO序列化、sender实际入队消息和follower实际分支对象均需测试；固定原回归保留。相同测试字节先原类RED再候选GREEN。
- 原Jackson默认拒绝未知字段，因此新状态更新消息只认证全节点统一补丁组；不得声称混合版本滚动升级兼容。未设置标志的普通DTO不额外序列化null标志。

候选正向验收保持3000ms明确选举profile、原真实混合自然16业务与两次领导切换/snapshot/DB和SDK恢复、TCC成功回复丢失及持久重放。所有副本、原盘加载与实际3/4回调须直接绑定原13已确认更新context和原SQL token；最终余额900、Undo空、确认10/效果1且三个副本清理。

源/依赖/父证据/覆盖字节、进程与配置/实际类来源、原始帧、SQL、快照元数据与JFR均闭合；仅自有D/E目录、端口和容器。未获得实际复现/正向结果前不得把源码风险升级为已复现或已修复。原参考问题、1000ms分票、metadata/logging、全配置/并发/分区/损坏/崩溃、通用TCCFence、指标/Exporter、生产认证、正式Rust和完整WP00仍分别验收。
