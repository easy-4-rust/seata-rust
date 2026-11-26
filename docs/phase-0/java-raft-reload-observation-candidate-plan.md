# Java RAFT 异步恢复完成观察候选

延续 java-retry-timeout-retention-candidate-plan.md、REC-TIMEOUT-001–004 与 ADR-0008。现有 0002 候选和 FILE 证据保持封存；新增独立 0003 补丁只观察 onLeaderStart 异步 SessionHolder.reload 的真实结果，不改变领导判断、客户端协议、TC 重试责任或正式 Rust/WP00 门禁。

验收行为：未执行或失败的 reload 不得产生成功标记；成功标记携带捕获的领导任期；旧任务晚完成不得覆盖较新成功任期；调用者同时核对当前领导身份、当前任期与成功任期。失败须记录 group、term 与异常。标记不等于服务就绪或失败隔离，不能据此声称异常领导已停止服务。

先用原实现运行测试得到行为 RED，再加入最小观察实现并运行目标与受影响回归。独立运行真实三节点 TC：Begin 对照、CommitRetryTimeout16、RollbackRetryTimeout17；每组两次领导切换和一次旧节点快照重启。确认原 XID/分支、复制进度、当前任期成功标记、锁语义及 native/console 查询；不能用进程启动、HTTP200 或未见异常代替完成证据。16 清除残留锁，17 默认保留锁；额外解锁配置另按实测说明。

输入、依赖、覆盖类、原 JAR、进程身份和所有结果可追溯；使用自有 D/E 目录和保留端口，退出清理自身进程。原版 RAFT 恢复、元数据/日志问题继续开放。seeded 会话不能称作自然超时或真实 RM 补偿。
