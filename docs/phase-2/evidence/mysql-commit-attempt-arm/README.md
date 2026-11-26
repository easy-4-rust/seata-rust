# MySQL 原提交尝试持久预置现场

2026-11-24 在自有 Windows MySQL 8.4.13、回环 43318 和隔离库 `undo_commit_attempt_20261124_a1` 验证 006 迁移。Rust [1.99](preconditions-199.log) 与 [1.85](preconditions-185.log) 各运行 `live_commit_preconditions` 的 29 项普通真实 MySQL 测试，均为 29/29 通过；另各运行 1 项 `experimental_changed_commit_server_ok_loss_is_unknown_with_durable_witness`，均为 1/1 通过。后者代理先放行独立控制事务 COMMIT，再截留原业务 COMMIT 的服务端 OK；原业务、Normal Undo 和 004 见证持久存在，客户端保守返回 Unknown。Rust 1.85 的[原始测试输出](proxy-test-185.log)与[代理输出](proxy-185.log)已复制到本目录；两工具链还通过本目标严格 Clippy。

正向测试检查 006 原 XID、BranchId、资源 ID、应用数据、锁键、JDBC 配置、响应码及 `Armed` 状态，原业务事务只发送一次 COMMIT；此前新增的独立控制 COMMIT 单独计数。负向测试分别在缺失 006、将 006 改成 MyISAM、预置同 XID/BranchId 但不同资源 ID 时拒绝发送业务 COMMIT，并显式确认原业务事务回滚。原 Java TC 双工具链成功提交与报告丢失的四次运行也保存独立 006 查询，见[原 TC 联试证据](../java-tc-changed-commit-report/README.md)。

固定原 Java TC 的 006 控制 COMMIT 确认丢失现已补测。Rust 1.99/1.85 各自验证了 006 已持久化但原确认未送达，业务连接零 COMMIT 请求并收到明确的回滚确认。后续以同一原身份持久围栏并完成原 TC 回滚。[固定原 TC 联试](../java-tc-changed-commit-report/README.md#006-控制提交确认丢失)。

固定原 Java TC 同样补测了 006 控制 COMMIT 未到达 MySQL。Rust 1.99/1.85 都确认本地业务回滚、无业务 COMMIT、恢复返回 `MissingAttempt`，原 TC 回调把围栏和回滚终态原子提交。006 最终为空。[请求截留联试](../java-tc-changed-commit-report/README.md#006-控制提交请求未到达-mysql)。

以上首轮证据只证明 `Armed` 的发送前持久交接和故障保守分类；下述后续恢复证据另行核验原 Undo 围栏及进程退出窗口。

2026-11-24 后续恢复增量：Rust [1.99](recovery-preconditions-199.log) 与 [1.85](recovery-preconditions-185.log) 各通过 33/33 项真实 MySQL 提交前置条件用例；需专用代理的 COMMIT OK 丢失用例单独排除，前次已双工具链通过。新增独立连接读取 006 的身份、资源、锁键、响应码与配置；篡改持久资源 ID 后读取明确拒绝。已提交原业务通过 004 见证及 Normal Undo 返回 `Applied`；原连接被杀导致业务回滚时，恢复者持久写入原 Undo guard，独立检查 guard=1、见证=0 才返回 `NotAppliedFenced`。

等待旧事务的两种反例也在双工具链通过：[回滚竞态 1.99](armed-old-owner-199.log)／[1.85](armed-old-owner-185.log) 在原事务仍持有 Undo 时先得到 Unknown，确认回滚后才得到持久围栏；[提交竞态 1.99](armed-old-commit-199-correct.log)／[1.85](armed-old-commit-185.log) 在恢复等待时由原连接提交，恢复转为 `Applied` 且不写入 guard。若把“见证暂为零”直接当作 `NotAppliedFenced`，回滚竞态中的首次 Unknown 断言会失败。

固定原 Java TC 的并发恢复也已覆盖。Rust 1.99/1.85 在原业务进程仍持有原事务时实际启动恢复，独立 SQL 确认恢复连接等待原业务连接的 004 主键行锁。同一次恢复在原业务回滚后得到 `NotAppliedFenced`，在原业务提交后得到 `Applied`，并分别完成原 TC 的回滚与提交。[固定原 TC 联试](../java-tc-changed-commit-report/README.md#原业务会话持锁时并发恢复)。

[子进程故障 1.99](armed-crash-parent-199.log)／[1.85](armed-crash-parent-185.log) 在子进程独立保存 Armed、仍持有原业务事务时终止该子进程；父测试进程随后重新连接，读取原 006 记录，持久围栏并独立核对业务=0、guard=1、见证=0。此证据覆盖 Armed 后原业务 COMMIT 前退出，不覆盖 COMMIT 已发送后的进程退出、围栏 COMMIT 回包丢失、恢复查询故障或原 Java TC 全链路终态。接口继续受默认关闭的 `experimental_changed_commit` 保护，WP05/WP06 Gate 未宣称完成。

原业务 COMMIT 服务端已回 OK、代理扣留回包时，恢复接口现另在 Rust [1.99](recovery-proxy-test-199.log)／[1.85](recovery-proxy-test-185.log) 各通过 1/1：调用方保留 Unknown 回执，恢复按 006/004 与 Normal Undo 返回 `Applied`，未将暂时缺失的客户端确认误判为 `NotAppliedFenced`。代理[1.99](recovery-proxy-199.log)／[1.85](recovery-proxy-185.log) 分别放行先前控制 COMMIT，只扣留原业务 COMMIT 的 OK。本次不包含业务 COMMIT 已发送但服务端未生效后的进程退出场景。

有效削弱对照另在 Rust [1.99](recovery-normal-without-witness-199.log)／[1.85](recovery-normal-without-witness-185.log) 各通过 1/1：测试在已经确认业务与 Normal Undo 持久提交后，临时将 004 见证位改为零。恢复观察先得零，原 Undo 唯一键围栏却读到 Normal；再次观察仍为零时只返回 Unknown 错误，数据库仍有 Normal Undo，没有生成 guard。恢复见证位后再执行后续断言。这使仅凭“见证为零”错误返回 `NotAppliedFenced` 的削弱实现失败。

后续故障矩阵又由 Rust [1.99](recovery-preconditions-199-crash-matrix.log)／[1.85](recovery-preconditions-185-crash-matrix.log) 各通过 35/35 项普通真实 MySQL 用例，四项专用代理用例另行运行。恢复查询使用的独立连接池关闭时，[1.99](recovery-query-failure-199.log)／[1.85](recovery-query-failure-185.log) 均先返回 Unknown 且没有 guard；换用可用连接后才持久围栏。

固定原 Java TC 的恢复查询故障也已补测。原 004 行锁使首次真实查询在服务端超时，Rust 1.99/1.85 均保持未知且无围栏写入。解除锁后才持久化一次围栏，同一原分支完成回滚。[固定原 TC 联试](../java-tc-changed-commit-report/README.md#004-恢复查询超时)。

围栏 COMMIT 本身在服务器回 OK、代理扣留回包时，[1.99](fence-recovery-loss-199-a.log)／[1.85](fence-recovery-loss-185-a.log) 各通过 1/1：首次恢复只返回 Unknown；独立 MySQL 查询已见 guard=1、业务=0，第二次按同一持久身份恢复才返回 `NotAppliedFenced`。两次分别由[代理 1.99](fence-proxy-199-a.log)与[代理 1.85](fence-proxy-185-a.log)只截留原围栏 COMMIT 的 OK，测试核对仅有一次该控制 COMMIT。

固定原 Java TC 也已补测。Rust 1.99/1.85 均在原业务进程退出后首次返回 `FenceCommitOutcomeUnknown`。独立连接确认唯一围栏持久化后重新检查，得到 `NotAppliedFenced`，期间零 BranchReport。原 TC 按原分支回滚且只提交一次二阶段终态。[固定原 TC 联试](../java-tc-changed-commit-report/README.md#undo-围栏-commit-确认丢失)。

另用真正不同的测试子进程覆盖原业务 COMMIT 包已发送后的两侧：[服务器已提交 1.99](child-business-recovery-199-a.log)／[1.85](child-business-recovery-185-a.log) 在服务端回 OK、代理扣留回包时终止子进程，新进程以原 006/004 返回 `Applied`，独立核对业务=1、Normal=1、见证=1、guard=0；[服务器尚未收到 1.99](child-precommit-recovery-199-a.log)／[1.85](child-precommit-recovery-185-a.log) 在代理收到原 COMMIT 包但未转发时终止子进程，新进程等待原连接终结并围栏，独立核对业务=0、guard=1、见证=0。代理分别记录[已提交 1.99](child-business-proxy-199-a.log)、[已提交 1.85](child-business-proxy-185-a.log)、[未转发 1.99](child-precommit-proxy-199-a.log)、[未转发 1.85](child-precommit-proxy-185-a.log)。这些仍是受控 MySQL 夹具，不替代固定原 Java TC 的跨进程帧/终态联试或外部写入器锁安全认证。

一次额外的否定判定反例揭示并修复了原 004 行缺失的问题：原实现将缺行等同于见证零，进而在 guard 可写时错误返回 `NotAppliedFenced`；[红灯](missing-witness-red-199.log)实际得到该错误结果。恢复入口现要求原 004 行存在且身份一致；普通 `first_phase_commit_observed` 的历史“缺行暂未观察到”语义保留。修复后 Rust [1.99](missing-witness-green-199.log)／[1.85](missing-witness-green-185.log) 均返回 Unknown 且不写 guard；两工具链各 [36/36](recovery-preconditions-199-witness-guard.log)／[36/36](recovery-preconditions-185-witness-guard.log) 普通现场回归通过。四项专用代理用例保持独立运行。

2026-11-25 增加 007 `report_started` 持久门闩后，旧 006 行迁移默认值 1 与缺失 007 时业务 COMMIT 前明确回滚分别由双工具链现场测试。受控原报告回包丢失后，新恢复报告入口只返回 `AlreadyStarted`，独立 MySQL 查询确认门闩为 1。首次 38 项聚合回归的[失败记录](report-gate-preconditions-199.log)揭示旧提交竞态夹具手工预置 006 行时未显式填写新行必需的 0，导致安全拒绝；修正夹具后 Rust [1.99](report-gate-preconditions-199-green.log) 与 [1.85](report-gate-preconditions-185-green.log) 各通过 38/38 项普通真实 MySQL 用例。四项需专用 COMMIT 代理的用例本轮未重新运行，其先前证据仍按原故障窗口保留；007 门闩自身 COMMIT 回包丢失与确认后发包前进程退出尚未验证。

007 门闩控制事务的服务端 OK 已发、回包被代理截留这一故障现由 Rust [1.99](report-claim-ok-loss-199.log) 与 [1.85](report-claim-ok-loss-185.log) 各通过 1/1。代理的[1.99 确认](report-claim-server-ok-199.txt)／[1.85 确认](report-claim-server-ok-185.txt)证明 COMMIT 到达服务端；独立 MySQL 查询见 `report_started=1`、业务=1、Normal Undo=1，客户端仅返回 `Claim` 未知，受控原报文对端未收到 `PhaseOne_Done`。随后按同一持久身份再次调用只返回 `AlreadyStarted`，仍无报告；`mysql.general_log` 仅新增一次门闩 COMMIT。该场景独立于已验证的原业务 COMMIT 回包丢失和 Undo 围栏 COMMIT 回包丢失。尚缺门闩已确认但发包前终止进程，以及固定原 Java TC 在该故障窗口的终态观察。

门闩已确认但原报告尚未送达的另一侧窗口现有双层证据：先在同进程取消等待 RM 注册的报告 Future，门闩仍为 1、原对端零报告、再次调用 `AlreadyStarted`；再让独立子进程读取 006/004 并确认门闩，停在未获响应的 RM 注册，父进程观察持久 `report_started=1` 后终止子进程。新连接恢复得到 `Applied`，但报告只返回 `AlreadyStarted`，原受控对端仍零报告、业务与 Normal Undo 均为 1。真实子进程证据见 Rust [1.99](report-claim-child-exit-199.log)／[1.85](report-claim-child-exit-185.log)，双工具链各 [41/41](report-gate-preconditions-199-child-exit.log)／[41/41](report-gate-preconditions-185-child-exit.log) 普通真实 MySQL 回归通过；五项专用代理用例仍按独立运行结果统计。这证明本地发送门闩在退出后拒绝重放，未替代固定原 Java TC 在此窗口的全局终态、RM 报告恰好一次交付或外部锁安全验收。

两个独立 `MySqlDataSourceManager` 连接池同时竞争同一 007 行的现场测试在 Rust [1.99](report-claim-concurrent-199-independent.log) 与 [1.85](report-claim-concurrent-185-independent.log) 各通过 1/1。只有一个恢复调用取得发送权，另一个返回 `AlreadyStarted`；受控 RM 对端只收到一条原成功报告，持久行 `report_started=1`，业务与 Normal Undo 各为 1。固定原 Java TC 的对应竞争结果另见[联试记录](../java-tc-changed-commit-report/README.md)。两个竞争者位于同一进程，不覆盖跨恢复者进程竞争或门闩取得后报告交付保证。

固定原 Java TC 的门闩确认后发包前退出已补齐双版本现场。独立子进程收到 MySQL OK 后退出，原回滚回调完成补偿，TC 端零分支报告。[原 TC 联试](../java-tc-changed-commit-report/README.md#007-门闩确认后发包前退出)。

固定原 Java TC 已补测原业务 COMMIT 被代理截留时的进程退出。006 持久、原 MySQL 会话独立确认终止后，恢复给出 `NotAppliedFenced`。原 TC 通过新 RM 连接完成一次回滚，原分支零报告。见[请求截留联试](../java-tc-changed-commit-report/README.md#006-已持久化且业务-commit-被截留后进程退出)。固定原 Java TC 也已补测 MySQL 原 OK 被截留后终止业务进程。原物理会话消失后，按 006/004 与 Normal Undo 恢复为 `Applied`，一个成功报告后原 TC 完成原分支提交回调。见[响应截留联试](../java-tc-changed-commit-report/README.md#业务-commit-已持久化且-ok-交付前进程退出)。

固定原 Java TC 的执行中退出场景已补测。Rust 1.99/1.85 均确认进程退出前后服务端仍处理同一原 COMMIT。随后原物理会话消失，独立连接查到业务、Normal Undo 与 004 已持久化，恢复端得到 `Applied` 并完成一次报告和原回调。[执行中退出联试](../java-tc-changed-commit-report/README.md#mysql-执行原业务-commit-期间进程退出)。
