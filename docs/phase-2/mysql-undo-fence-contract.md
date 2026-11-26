# WP05 原唯一身份的持久防悬挂围栏

细化既有[WP05](wp05-mysql-at-plan.md)、[同事务写入](mysql-undo-store-contract.md)及RM恢复契约，不改变原十四项与WP06验收。原`undo_log(xid,branch_id)`唯一键负责仲裁晚到的Normal写入；不能以一次查询无行证明旧请求永久不生效。

1. `MySqlDataSource::fence_undo_log(xid,branch_id)`是独立控制事务。固定原身份、同一schema、有限连接与一个绝对期限；不执行/重放业务SQL，不提交调用者业务事务，不修改/删除已有Normal行。校验现有Undo表、权限可见性及InnoDB提交刷盘配置后，按原身份`SELECT ... FOR UPDATE`。
2. 有Normal行返回`UndoPresent`，只表示需要后续读取/补偿处理，不代表业务历史提交或二阶段已执行。无行时写原GlobalFinished=1、Jackson/NONE context与原默认内容`{}`，仅在控制COMMIT明确完成且未过期后返回`Fenced{inserted:true}`。已有完整、受支持的GlobalFinished记录按只读事务核对后返回`inserted:false`，不更新时间或重写内容。状态和内容均使用锁定的当前读取；RR旧快照不得使刚提交guard的第二次内容读取丢失。未知状态、畸形guard、未认证配置保持错误。
3. `Fenced`只证明该原身份上的晚到Normal插入受到已提交唯一记录阻止；它不区分此前已补偿与从未提交，不能直接映射为通用RM的Applied/NotApplied。资源适配器、完整Undo处理与持久结果记录仍须完成。围栏不得自动过期或删除；后续GC须先证明旧尝试已终止。外部直接修改/删除控制记录不在这个事实的前提内。
4. 围栏COMMIT尝试前失败与COMMIT结果未知使用独立错误阶段，保留原身份，Debug不显示原XID。COMMIT错误、断连或超时一律不返回Fenced，不因客户端知道测试故障而推断结果；不自动重发。显式后续观察/围栏调用仍使用同一身份及唯一键，由调用方/TC协调恢复。
5. Future在任何await取消时拥有的实际事务/连接必须丢弃，不能在外层留下可继续使用的半操作。最终Ready也要复核绝对期限。已有记录只读路径与新guard写入路径分别处理，避免把未尝试控制COMMIT的错误误标成提交未知。
6. 真实MySQL验证：防悬挂持久可见、重复观察不改原七列、两个旧准备作用域被阻止、Normal先赢、并发guard仲裁、实际锁等待中取消/超时、无效身份/坏表/畸形guard及提交刷盘配置。独立MySQL原始包观察者分别丢弃已成功COMMIT回复、阻断COMMIT发送及延迟回复，核对实际COMMIT/INSERT次数、数据库事实、原身份和显式恢复，无业务SQL重放。原Java GlobalFinished写入/正常写入冲突对照单独留证据。

当前MySQL8.4本机夹具与明确配置限定现场范围；生产TLS、主从/多节点、完整原测试和控制记录GC继续原计划。业务公开COMMIT、真实TC登记绑定、提交未知全链路、WP06补偿、原TC排他锁覆盖问题仍OPEN；不接受外部TC风险，共享锁继续暂缓。

## 004 恢复配置下已消费日志的防悬挂

使用 004 一阶段见证表的受管理 RM 回滚，在消费 Normal Undo 后以同一原 XID 和分支号留下原格式的 GlobalFinished 记录。补偿、Normal 根与子片删除、围栏写入及恢复终态使用同一控制事务。旧事务对同一唯一身份的后续 Normal 写入必须失败。普通数据库回滚、旧恢复表版本和原 Java 兼容的独立 Undo 补偿沿用各自既有行为。围栏与原一阶段提交见证的历史含义独立。

部署前已存在的回滚终态及其 Undo 记录仍须按历史数据单独核验。本次围栏写入发生于新执行的补偿事务；已经返回终态的重复回调仍沿用终态读取。

真实 MySQL 首先复现了回滚回调成功后旧事务可再次写入 Normal。修复后，双工具链现场都核对了旧写入收到 1062、业务行归零、重复回调保留围栏，以及 MySQL 重启前后围栏和恢复终态一致。提交见证历史继续可观测。 [受管理回滚围栏记录](wp05-mysql-managed-rollback-fence.json)

### 受管理回滚的控制 COMMIT 故障

固定 MySQL 8.4.13 下，丢弃 MySQL OK、阻断 COMMIT 发送、延迟 OK 至 RM 期限之后三类故障均在两套 Rust 工具链现场验证。首次回调的分支状态为 `PhaseTwoRollbackFailedRetryable`，日志保持 `Pending`。独立 SQL 在允许同一分支重试前确认实际数据库状态。丢回包和延迟回包时，补偿已经提交，重试从持久终态收敛。阻断 COMMIT 时，首次事务退出后原业务行及 Normal Undo 仍在，重试才提交补偿事务。每例只有一次实际补偿事务提交。

原始 MySQL 包分别核对补偿、Normal 删除、围栏写入、终态更新和 COMMIT 的原连接及顺序。六次运行的围栏、终态和历史见证在重启后保持一致。本次证据使用受控 TCP 回调。完整原 TC 故障链路和公开业务 COMMIT 仍按原验收范围推进。 [三种控制提交故障验证](wp05-mysql-managed-rollback-fault.json)
