# 固定原 Java TC 有变化 COMMIT／报告联试准备

本步骤把前一轮实验提交／成功报告接入已有的固定原 Java TC、DB 锁存储和正式 Rust TM/RM 夹具。新增 `live_commit_check_java` 现场用例由原业务事务实际提交业务行、Normal Undo 和 004 见证，使用 005 资源绑定及组合锁配置，然后向原 TC 发送一次成功报告。测试在全局提交前核对原锁行，在原 TC 回调后核对业务保留、Undo 清理、见证和回调终态。`tools/formal-first-phase-commit/java_tc_report.py` 复用固定 Java pin/JAR/Connector/J、隔离认证夹具与双向原始帧代理，并独立核对报告身份与最终数据库计数。

此前现场联试尚未运行：Rust 1.99 和 1.85 的目标测试均 `--no-run` 编译通过；各自开启实验特性的全目标严格 Clippy 通过，Python runner 的 AST 解析与 `cargo fmt --all -- --check` 通过。原始日志在本目录。随后尝试启动自有 Ubuntu-20.04 WSL MySQL 容器，Docker 初始化出现 `SIGBUS`/I/O 错误；D 盘一度只剩约 0.4 GiB。使用 `cargo clean -p seata-db-mysql` 清理本项目可重建产物后，Ubuntu 与 Alpine 的 WSL `CreateInstance` 均以 `HCS_E_CONNECTION_TIMEOUT` 退出。Docker Desktop CLI 查询亦未完成，已停止该查询进程。Ubuntu 发行版在最终观察时处于 Stopped；自有容器可能仍留在其隔离 Docker 数据根中，待运行时恢复后按精确 ID/标签清理或复用。

一次额外 `cargo test -p seata-db-mysql --features experimental_changed_commit` 因 D 盘再次耗尽而在链接其他集成测试目标时失败，见 `java-changed-report-offline-1.85.log`；不计为回归通过。之后再次仅对本项目 MSRV 目标执行 `cargo clean -p seata-db-mysql`，回收可重建产物。源码未因此回退。

2026-11-24 改用 D 盘[自有 Windows MySQL 8.4.13](../mysql-native-prepare-20261124/README.md) 后，runner 在测试前核对 `VERSION()`、端口 43318、数据目录和原 Java pin/JAR/Connector/J SHA；每次新建隔离数据库并加载固定原 TC/Undo DDL。首次 Rust 1.99 运行暴露测试认证插件缺少精确资源名启动参数：原 TC 拒绝 RM 注册，准备阶段保守地返回无回执期限错误。补入 `seata.callback.fixture.resource` 后，Rust [1.99](run-1.99-8bd86af9/verified.json) 与 [1.85](run-1.85-784bc600/verified.json) 各通过 1/1 固定原 Java TC 有变化 COMMIT／成功报告现场用例。

两次均有单次原 BranchRegister、单次 `PhaseOne_Done`、单次 GlobalCommit、原 TC 回调完成；最终 `global_table/branch_table/lock_table/undo_work/undo_log/first_phase_committed/commit_terminal` 计数为 `0/0/0/1/0/1/1`。原始帧、客户端和 TC 日志、固定身份/哈希、进程退出记录分别保留在上述 run 目录。认证插件是独立测试覆盖层，原 Java TC 源码与 JAR 未修改；MySQL 是 Windows 原生实例，不能冒称固定 Linux 镜像认证。

2026-11-24 追加“已登记本地事务明确回滚后报告失败”用例。业务写入和 Normal Undo 保持未提交，测试先独立观察原 TC 锁行，再在原连接确认回滚后发送一次 `PhaseOne_Failed`；原 TC 返回同请求 ID 的成功响应，然后 Rust TM 发起 GlobalRollback。双工具链及改进后的 runner 复跑如下：

| Rust | 成功提交与 `PhaseOne_Done` | 明确回滚与 `PhaseOne_Failed` | Undo 表安全拒绝后自动报告 | `enroll` 表安全拒绝后自动报告 | 真实 Undo 唯一键冲突 |
|---|---|---|---|---|---|
| 1.99 | [1/1](run-commit-1.99-899274f9/verified.json) | [1/1](run-failed-1.99-27b1528a/verified.json) | [1/1](run-undo_error-1.99-b877a2e8/verified.json) | [1/1](run-enroll_error-1.99-5d841b1e/verified.json) | [1/1](run-undo_duplicate-1.99-a342ffda/verified.json) |
| 1.85 | [1/1](run-commit-1.85-6652707f/verified.json) | [1/1](run-failed-1.85-f09a27a8/verified.json) | [1/1](run-undo_error-1.85-e05f1bbc/verified.json) | [1/1](run-enroll_error-1.85-4b6c9c6f/verified.json) | [1/1](run-undo_duplicate-1.85-59781b13/verified.json) |

明确回滚路径的原 BranchRegister、失败报告及 GlobalRollback 各恰好一次；报告前原锁行为 1、业务与 Undo 对其他连接不可见，终态 `global/branch/lock/business/undo/first_phase_committed/commit_terminal` 全为 0。另两路在自有数据库的 Undo 表或控制表设置测试触发器，分别触发 `MySqlPrepareError::Undo` 与 `Enrollment` 的表安全检查拒绝：两者均保留原 TC 回执，确认原连接回滚后自动发送一次 `PhaseOne_Failed`。Undo 表拒绝时控制行数为 1，控制表拒绝时为 0；两者均收到原 TC 同请求 ID 的报告成功响应，GlobalRollback 后七项终态计数全为 0。**触发器在实际 INSERT 前被元数据安全检查拒绝，不能把这两路证据称为数据库写入错误。** 前次 Undo 夹具用预处理语句协议创建触发器时被 MySQL 拒绝，改用原始 DDL 后双版本通过；该夹具错误未计为产品故障。

真实写入故障另用控制表的临时测试锁暂停 `enroll`，等待原 TC 将 `BranchRegister` 的真实 BranchId 写入 `branch_table`；独立连接以该原 XID/BranchId 预置一条 defense Undo，释放锁后原事务在 `Undo insert` 收到 MySQL SQLSTATE `23000`、错误码 `1062`。Rust 保留同一回执，显式回滚成功后自动发送一次失败报告；原 TC 同请求 ID 成功响应并执行 GlobalRollback。最终 `global/branch/lock/business/undo/first_phase_committed/commit_terminal` 为 `0/0/0/0/1/0/0`，唯一保留的是测试预置的冲突 Undo。Rust 1.99/1.85 分别取得独立证据，未靠猜测 BranchId 或代理合成注册响应。上表各 run 目录保存原始 RM/TM 帧、TC 日志、客户端输出、数据库事实与进程清理记录。两工具链目标严格 Clippy、格式检查和 runner Python AST 解析通过。原 TC 的控制登记真实写入故障、驱动断连与回滚丢响应仍缺。

报告响应丢失另由 [Rust 1.99](run-report_loss-1.99-5b05f598/verified.json) 与 [Rust 1.85](run-report_loss-1.85-d50c01bf/verified.json) 各运行 1/1：原 TC 收到单次 `PhaseOne_Failed` 后发出成功的 `BranchReportResponse`，帧代理记录同请求 ID 的原始成功字节，再各截留一次并关闭对应连接。客户端返回报告结果未知，保留已确认的原本地回滚与真实 BranchId；后续 GlobalRollback 完成且最终七项计数全为 0。原始帧中 `drop_original_report_reply` 各恰好一次、交付数为 0，观察器禁止第二次报告并正常退出。新增代理模式后，普通透传路径也由 Rust 1.99 [复跑通过](run-failed-1.99-998ca639/verified.json)。此证据覆盖已准备分支的明确回滚／失败报告回包丢失，尚不覆盖准备内部错误报告、成功报告回包丢失或跨进程恢复。

各次 runner 的 Java TC、TM/RM 帧观察器和 Rust 客户端进程均正常退出；测试结束后使用 `mysqladmin shutdown` 关闭自有实例。原 Java TC 对所有外部写入器的排他锁覆盖、旧尝试持久围栏、其余报告丢失与恢复及完整 WP05/WP06 仍开放。

2026-11-24 又验证了成功报告的原回包丢失：[Rust 1.99](run-commit_report_loss-1.99-dc3dd507/verified.json) 与 [Rust 1.85](run-commit_report_loss-1.85-59bd8021/verified.json) 各通过 1/1。原 TC 收到一次 `PhaseOne_Done` 并发回同请求 ID 的成功字节，RM 代理记录后只截留这一次回包；Rust 将报告结果分类为未知，同时保留真实 BranchId 与已独立确认的本地 COMMIT，不重发报告。随后 TM 单次 GlobalCommit，原 TC 回调完成，最终七项计数均为 `0/0/0/1/0/1/1`。两次原帧记录的报告交付数为 0，进程清理完成。重构后的正常透传用例由 Rust 1.99 [复跑通过](run-commit-1.99-9322515e/verified.json)。这覆盖已准备分支的成功报告回包丢失；准备内部错误后的报告丢失、跨进程恢复、外部锁覆盖与旧尝试持久围栏仍待验证。

2026-11-24 补测准备内部错误后的自动失败报告回包丢失：[Rust 1.99](run-undo_report_loss-1.99-7ce58d02/verified.json) 与 [Rust 1.85](run-undo_report_loss-1.85-e16f9238/verified.json) 各通过 1/1。测试触发器使 Undo 表在 INSERT 前被安全检查拒绝，故这是 `Undo` 准备错误，不是实际写入故障。原连接显式回滚确认后，Rust 对真实 BranchId 只报告一次 `PhaseOne_Failed`；原 TC 的同请求 ID 成功响应由代理记录并截留。错误对象保留原准备错误、回滚事实和回执，报告结果为未知，再次报告被门闩拒绝。原 TC 随后 GlobalRollback，七项终态计数全为 0；正常透传路径由 Rust 1.99 [复跑通过](run-undo_error-1.99-02a3ca97/verified.json)。这仅覆盖 Undo 表安全拒绝后的自动报告；控制登记真实写入错误、其报告回包丢失和跨进程恢复仍待验证。

2026-11-24 再补控制登记的真实唯一键冲突：[Rust 1.99](run-enroll_duplicate-1.99-60ee7335/verified.json) 与 [Rust 1.85](run-enroll_duplicate-1.85-107ac708/verified.json) 各通过 1/1。测试先锁住自有控制表，在原 TC 完成单次 BranchRegister 后从原 `branch_table` 读取真实 BranchId，给同一 XID/BranchId 预置回调归属行。释放表锁后，Rust 的控制登记 INSERT 在 MySQL 的 `statement/com/Execute` 事件中实际产生 SQLSTATE `23000`、错误码 `1062`；[原始错误事件](run-enroll_duplicate-1.99-60ee7335/mysql-error-1062.tsv)与[另一工具链事件](run-enroll_duplicate-1.85-107ac708/mysql-error-1062.tsv)独立保留。恢复层按原设计读取既有行，将其分类为 `CallbackObserved`，不覆盖回调归属；准备层保留原回执，显式回滚后单次报告 `PhaseOne_Failed`。报告前核对预置行仍在，之后只移除测试预置行，再由原 TC 执行 GlobalRollback；最终七项计数全零。Rust 1.99 的[普通控制表安全拒绝路径](run-enroll_error-1.99-bf630ed4/verified.json)复跑通过。首次验收脚本误以 SQL_TEXT 可用于过滤二进制预处理 Execute，客户端测试已过但独立断言失败，记录在 `run-enroll_duplicate-1.99-9dbcffd3`；修正为数据库、事件类型、错误码和 SQLSTATE 后双工具链完整通过。此证据证明控制登记实际写入冲突及保守归属处理；其他驱动错误、报告回包丢失和跨进程恢复仍待验证。

同一真实控制登记冲突的报告回包丢失另由 [Rust 1.99](run-enroll_duplicate_report_loss-1.99-698bfd8a/verified.json) 与 [Rust 1.85](run-enroll_duplicate_report_loss-1.85-5870da10/verified.json) 各通过 1/1。两次 MySQL `statement/com/Execute` 的[1062／23000 记录](run-enroll_duplicate_report_loss-1.99-698bfd8a/mysql-error-1062.tsv)与[第二工具链记录](run-enroll_duplicate_report_loss-1.85-5870da10/mysql-error-1062.tsv)独立保存；原 TC 接收一次 `PhaseOne_Failed` 并发出同请求 ID 成功响应，代理只截留该回包并关闭连接。Rust 的错误对象仍保留 `CallbackObserved` 准备错误、真实 BranchId 和已确认本地回滚，报告结果为未知，单次门闩拒绝第二次报告。原 TC 随后 GlobalRollback，最终七项计数全零；Rust 1.99 的[正常回包路径](run-enroll_duplicate-1.99-04bb8a6a/verified.json)复跑通过。此证据覆盖该控制登记冲突的报告回包丢失，仍不代替其他写入错误、跨进程恢复和生产锁安全门禁。

2026-11-24 增加 006 提交尝试表后，原 Java TC 成功提交分别由 Rust [1.99](run-commit-1.99-ec4d0375/verified.json) 与 [1.85](run-commit-1.85-f3782875/verified.json) 通过，成功报告回包丢失分别由 [1.99](run-commit_report_loss-1.99-9fa93428/verified.json) 与 [1.85](run-commit_report_loss-1.85-02b76e5b/verified.json) 通过。四次运行各保存独立 MySQL 查询的 `commit-attempt.tsv`；runner 逐字节核对原 XID、BranchId、资源 ID、锁键和 `Armed` 状态，并保持原 TC 一次注册、一次报告及原回调终态核验。记录在独立控制事务提交后，业务、Normal Undo 与 004 见证才可由原业务事务提交。这证明本地持久交接顺序及原身份一致性，不证明进程退出后的恢复者已能安全给出 `NotAppliedFenced`。

2026-11-24 新增固定原 Java TC 的已提交恢复后报告场景：Rust [1.99](run-commit_recovered-1.99-7f1a33da/verified.json) 与 [1.85](run-commit_recovered-1.85-8265d864/verified.json) 各通过 1/1。原连接提交后，测试以原 XID/BranchId 调用 `recover_commit_attempt`，确认返回 `Applied`，再使用原 `RegisteredBranch` 回执向 Java TC 单次报告 `PhaseOne_Done`。两次 runner 均核对原 RM 单次注册及报告、TM 单次 GlobalCommit、原 TC 回调、原成功报告响应、006 原身份、业务=1、Undo 清理=0、004 见证与提交终态。原始 RM/TM 帧、TC 日志、独立 MySQL 查询和测试输出保存在对应 run 目录。这是同进程恢复调用与固定 TC 的联试；进程退出后重建 RM/TM、凭持久身份恢复原报告和全局终态仍需独立验证。

2026-11-25 补充真正跨进程的 **RM 已确认本地提交后退出** 场景：Rust [1.99](run-commit_cross_process-1.99-84e28f37/verified.json) 与 [1.85](run-commit_cross_process-1.85-a54865ed/verified.json) 各通过 1/1。子进程使用原 Java TC 完成一次 `BranchRegister`，保存 006 `Armed`、提交业务/Normal Undo/004 后直接退出，不执行 RM shutdown；父测试进程从自有 MySQL 重新读取 006 原身份、确认 `Applied`、新建 RM 连接后向同一原 TC 单次发送 `PhaseOne_Done`。TM 持续持有原全局事务并单次 GlobalCommit，重建的 RM 收到原 TC 回调并完成清理。runner 独立核对两条 RM 连接、第一条连接预期 reset、一次注册/报告、原报告成功响应、原 TM 帧、006 身份和最终 `global/branch/lock/business/undo/witness/terminal=0/0/0/1/0/1/1`；原始帧、子进程测试输出、TC 日志与数据库查询均在 run 目录。首次客户端测试通过但观察器把预期 reset 计为失败，见 `run-commit_cross_process-1.99-4540c346`；观察器改为只允许该场景第一条 RM 连接恰好一次 reset 后，双工具链完整通过。此证据不覆盖 TM 自身退出、原业务 COMMIT 未确认时的固定 TC 联试、跨进程报告门闩持久性或外部写入器锁覆盖；不能据此关闭 WP05/WP06 Gate。

2026-11-25 为正常与恢复后的成功报告共用 007 `report_started` 持久门闩。旧 006 行迁移时默认置 1，新提交尝试显式置 0；确认本地 `Applied` 后，必须先持久确认 0→1 才发出原报告。Rust [1.99](run-commit_cross_process-1.99-29d4d250/verified.json) 与 [1.85](run-commit_cross_process-1.85-1d4c57c1/verified.json) 的重跑各通过 1/1，runner 在[原数据库行](run-commit_cross_process-1.99-29d4d250/commit-attempt.tsv)与[第二工具链行](run-commit_cross_process-1.85-1d4c57c1/commit-attempt.tsv)独立核对 `report_started=1`；测试在原 TC GlobalCommit 前再次尝试同一恢复报告得到 `AlreadyStarted`，原始帧仍仅一次成功报告。正常提交、成功报告回包丢失、同进程恢复三类用例也由两工具链各重跑通过，分别见对应 `run-commit-*`、`run-commit_report_loss-*` 和 `run-commit_recovered-*` 的 2026-11-25 运行目录。受控原报文对端的报告丢包路径新增恢复重报否定断言，两工具链各通过。门闩 COMMIT 回包丢失、确认后发包前退出、TM 重建及外部写入器锁安全仍未现场认证。

2026-11-25 增加同一原分支的并发恢复报告竞争。业务 RM 子进程确认本地 COMMIT 后退出；父进程创建两个独立 MySQL 管理器及连接池，同时调用恢复报告入口。Rust [1.99](run-commit_cross_process_race-1.99-2dbf3341/verified.json) 与 [1.85](run-commit_cross_process_race-1.85-55327f44/verified.json) 各通过 1/1：一个调用获得 007 发送权，另一个返回 `AlreadyStarted`；runner 独立核对原 TC 一次注册、一次 `PhaseOne_Done`、一次 GlobalCommit、原回调完成，以及 `global/branch/lock/business/undo/witness/terminal=0/0/0/1/0/1/1`。共享测试重构后，未竞争的跨进程恢复也由 [1.99](run-commit_cross_process-1.99-117f334e/verified.json) 与 [1.85](run-commit_cross_process-1.85-53fb10f9/verified.json) 各重跑通过。这里的两个竞争调用处于同一父进程，使用独立连接池；尚未验证两个恢复者进程相互竞争、TM 重建或外部写入器锁覆盖。

2026-11-25 又将竞争者改为两个真正独立的恢复子进程，并由父测试在同一 TCP 屏障释放。最终代码的 Rust [1.99](run-commit_distinct_process_race-1.99-a86b61ec/verified.json) 与 [1.85](run-commit_distinct_process_race-1.85-d7bd60a0/verified.json) 各通过 1/1：两个不同 PID 的门闩结果为 `[1,2]`（发送成功／`AlreadyStarted`），原始 RM 帧各有一次 `PhaseOne_Done`、匹配请求 ID 的成功回包、一次 BranchCommit 回调及同连接同请求 ID 的成功响应；TM 帧各有一次 GlobalCommit。独立 MySQL 查询确认 007 `report_started=1`，最终 `global/branch/lock/business/undo/witness/terminal=0/0/0/1/0/1/1`，所有联试进程退出。修订后的单进程双连接池回归亦在 [1.99](run-commit_cross_process_race-1.99-c93c7749/verified.json) 与 [1.85](run-commit_cross_process_race-1.85-f7a0117e/verified.json) 重新通过。联试使用经哈希固定的原 Java TC JAR、Connector/J 与 MySQL 8.4.13 镜像；自有容器已删除。TM Launcher 在父进程持续存活，因而 TM 重建、外部写入器锁覆盖和 WP05/WP06 完整门禁仍开放。

可复验入口为 [`run_desktop.ps1`](../../../../tools/formal-first-phase-commit/run_desktop.ps1)：Docker Desktop 已运行且固定镜像已存在时，从仓库根目录分别执行 `& tools/formal-first-phase-commit/run_desktop.ps1 -RustVersion 1.99 -Scenario commit_distinct_process_race` 和 `-RustVersion 1.85`。脚本创建随机命名、仅回环绑定的自有 MySQL 容器，等待真实 TCP 握手后调用原 runner，并在退出时删除该容器。该入口的双版本实跑分别另见 [1.99](run-commit_distinct_process_race-1.99-95f81133/verified.json) 与 [1.85](run-commit_distinct_process_race-1.85-1728f226/verified.json)。

TM 重建边界补记：上述场景的 `TransactionManager` 与 Launcher 从 Begin 至 GlobalCommit 一直存活在父测试进程。新的 TM 不能凭这些 RM 证据取得 Launcher 结束权；持久来源和故障验收要求见[一阶段提交契约](../../mysql-first-phase-commit-contract.md#tm-重建与-launcher-权限边界待实现)。

### 007 报告门闩 COMMIT 回包丢失

真实本地业务事务先提交 SQL、Normal Undo 与 004 见证。恢复端确认 `Applied` 后更新 007 门闩。MySQL 报文代理按原 SQL 与请求连接定位这次更新，仅丢弃该控制事务的原始 COMMIT OK。代理关闭连接前，独立 SQL 已看到 `report_started=1`、原分支仍为 Registered、一个业务行、一个 Normal Undo 和一条 TC 锁。

Rust 将首次报告尝试分类为 `Claim` 未知，以原身份再次恢复得到 `AlreadyStarted`。原始 RM 帧无 BranchReport 请求或响应，原始 TM 帧只有一次主动 GlobalRollback。原 TC 向同一 XID/BranchId 发出一次回滚回调，日志表明分支经历 Pending → Completed。独立查询确认补偿删除业务行和 Normal Undo，原分支只留一条防悬挂记录，TC 会话、分支、锁清空，007 门闩仍为 1。双版本均通过，受控证据变体验证缺失、错误转发的 OK 和重复门闩写入会被拒绝。

[Rust 1.99](run-commit_report_claim_loss-1.99-b0799b37/verified.json) · [Rust 1.85](run-commit_report_claim_loss-1.85-a98e9346/verified.json)

复跑可在各版本下执行 `run_desktop.ps1 -RustVersion 1.99 -Scenario commit_report_claim_loss`。联试由仍持有原 XID 的 TM 主动回滚。门闩确认后、发包前独立进程退出见下一项联试。

### 007 门闩确认后发包前退出

独立 Rust 恢复进程从 006/004 读取原分支，提交 007 门闩。原 MySQL COMMIT OK 送达，原连接随后发出 QUIT。运行器将子进程的 RM 注册请求送到同一固定原 TC，收到成功响应并暂缓交付。独立查询确认门闩为 1、原分支仍为 Registered、业务行、Normal Undo 和锁各为 1。此后运行器终止并回收子进程，读取其关闭的 RM 连接，确认没有分支报告字节。

持有原 XID 的父进程重新恢复时得到 `AlreadyStarted`。原 RM 观察器确认零次 BranchReport，TM 发出一次 GlobalRollback，TC 向原分支发出一次回滚回调。原业务行和 Normal Undo 清除，留下原防悬挂记录，TC 会话、分支和锁均清空。两 Rust 版本各通过 1/1。用实际报文的副本验证了四种证据变体会被拒绝：缺少已转发的 COMMIT OK、缺少 QUIT、伪造子进程在 QUIT 前退出、伪造 RM 注册响应已交付。

[Rust 1.99](run-commit_report_claim_child_exit-1.99-e1d4e1c1/verified.json) · [Rust 1.85](run-commit_report_claim_child_exit-1.85-8c6ea4ac/verified.json)

使用 `run_desktop.ps1 -RustVersion 1.99 -Scenario commit_report_claim_child_exit` 可复跑。原 TM 仍由父测试进程持有，外部写入器准入和完整 WP05/WP06 生产验收继续开放。

### 006 已持久化且业务 COMMIT 被截留后进程退出

固定 Java TC 为独立 Rust 子进程分配原分支，并保留全局锁。子进程在业务连接写入事务内的 Normal Undo，在另一个控制连接取得 006 COMMIT 确认，然后发出业务 COMMIT。MySQL 代理按同一业务连接的原 Normal Undo 写入识别该 COMMIT，仅截留该请求。独立 SQL 确认原业务连接仍在，006 已持久、原 TC 锁仍在，业务行、Normal Undo 和 004 见证均未对独立连接可见。

运行器终止并回收子进程，独立确认原 MySQL 会话消失，再允许父进程按原身份恢复。恢复先持久化一条防悬挂记录，再返回 `NotAppliedFenced`。原 TC 收到零次 BranchReport。父进程仍持原 TM，重新注册 RM 后发出一次 GlobalRollback，新 RM 连接处理原分支的一次回滚回调。最终没有业务行或 Normal Undo，防悬挂记录保留，004 提交见证与 007 发送标记仍为零，TC 会话、分支、锁清空。

用原证据副本验证三个异常证据会被拒绝：出现已转发的业务 COMMIT、缺少 006 提交确认、缺少围栏提交确认。该场景覆盖客户端已发出、服务端尚未收到业务 COMMIT 时的进程退出。MySQL 已提交的窗口见下一项联试。

[Rust 1.99](run-armed_child_business_commit-1.99-da85069c/verified.json) · [Rust 1.85](run-armed_child_business_commit-1.85-6bdff135/verified.json)

可用 `run_desktop.ps1 -RustVersion 1.99 -Scenario armed_child_business_commit` 复跑。

### 业务 COMMIT 已持久化且 OK 交付前进程退出

原 Rust 业务子进程取得固定原 Java TC 的分支回执，并通过独立控制连接持久化 006。代理转发原业务 COMMIT，保留 MySQL 返回的原始 OK 字节并截留该响应。子进程和原物理 MySQL 会话仍在时，独立 SQL 查到原分支已持久化的业务行、Normal Undo 和 004 见证，TC 仍持原锁，007 门闩为零。

运行器终止并回收业务子进程，独立确认原 MySQL 会话消失。恢复端用同一原身份得到 `Applied`，通过替换 RM 成功送达一次 `PhaseOne_Done`，再次申请报告得到 `AlreadyStarted`。持有原 XID 的父进程发出一次 GlobalCommit，TC 对替换 RM 发出一次原分支提交回调。最终保留原业务行，清除 Normal Undo，维持 004 提交见证与 007 门闩。没有写入防悬挂记录，TC 会话、分支、锁均清空。

[Rust 1.99](run-armed_child_business_commit_ack-1.99-b496ab39/verified.json) · [Rust 1.85](run-armed_child_business_commit_ack-1.85-e64b0d5b/verified.json)

回放实际证据时，校验器拒绝了四种改动，包括伪造原 OK 已送达、替换原 OK 字节、将独立查询的提交见证改为零、让报告早于原 MySQL 会话关闭。可用 `run_desktop.ps1 -RustVersion 1.99 -Scenario armed_child_business_commit_ack` 复跑。MySQL 仍在执行原 COMMIT 时的窗口见下一项联试。

### MySQL 执行原业务 COMMIT 期间进程退出

使用固定原 Java TC 与由本次运行独占的 MySQL 8.4.13 容器。独立控制连接确认 006 后，代理捕获并转发唯一一条原业务 COMMIT。容器在这条请求转发前启用短时 binlog group commit 延迟，使独立 SQL 能分别在终止子进程前和回收子进程后看到同一物理会话的 `Query / COMMIT / waiting for handler commit`。两次查询均确认 TC 的原分支与锁仍在、007 为零，业务行、Normal Undo 和 004 见证尚未对独立连接可见。

原物理会话彻底消失后，独立 SQL 才查询到原业务行、Normal Undo 和 004 见证均已持久化，此时 007 仍为零。之后放行恢复端，按原身份得到 `Applied`。替换 RM 发送一次 `PhaseOne_Done`，同一原 TM 发起一次 GlobalCommit，原 TC 完成一次 BranchCommit 回调。最终保留业务行，清除 Undo，004 与 007 保持为 1，TC 会话、分支、锁均清空。

[Rust 1.99](run-armed_child_commit_in_flight-1.99-4b605754/verified.json) · [Rust 1.85](run-armed_child_commit_in_flight-1.85-49898abd/verified.json)

未启用延迟时，现场对照因原物理连接已回到 `Sleep` 且提交见证已可见而拒绝进入终止步骤。回放实际通过的证据时，校验器还拒绝了五种改动，包括退出前或退出后任一次执行状态显示为 `Sleep`、退出后的采样被移到回收之前、退出后缺少提交见证、伪造原 OK 已送达。可用 `run_desktop.ps1 -RustVersion 1.99 -Scenario armed_child_commit_in_flight` 复跑。完整外部写入器准入与 WP05/WP06 生产门禁继续验收。

### Undo 围栏 COMMIT 确认丢失

原业务进程在 006 持久化后发出业务 COMMIT。代理截留该请求，原进程被终止，独立 SQL 确认原 MySQL 会话消失。恢复端按原 XID 和 BranchId 插入一条防悬挂记录，并执行一次围栏 COMMIT。代理捕获 MySQL 的原始成功响应，截留后关闭连接。第一次恢复返回 `FenceCommitOutcomeUnknown`，仍携带同一原事务和分支身份。

恢复端暂停复查。独立 SQL 确认防悬挂记录已持久化，业务行、Normal Undo、004 见证与 007 门闩均为零，原分支及全局锁仍在。放行后，恢复端在新的 MySQL 连接锁定同一 Undo 键并读取原记录，得到 `NotAppliedFenced`。成功报告入口同样拒绝发送。两次检查只读已有围栏，均通过 ROLLBACK 释放检查事务，未发出新的围栏 COMMIT。

持有原 XID 的 TM 发起一次 GlobalRollback，原 TC 通过替换 RM 完成原分支回滚。持久日志只经历一次 Pending 到 Completed，原始 MySQL 报文只记录一次二阶段终态写入和成功 COMMIT。最终保留原围栏，TC 会话、分支和锁清空。

[Rust 1.99](run-armed_child_fence_ack_loss-1.99-02010e69/verified.json) · [Rust 1.85](run-armed_child_fence_ack_loss-1.85-02960315/verified.json)

正常传递围栏 OK 的现场对照直接得到 `NotAppliedFenced`，触发了新测试的首次结果断言。另一次原 TC 并发下发两条相同分支的回滚请求，原始帧回放确认两个响应都成功，MySQL 只有一次终态提交。证据副本中的错误响应交付、重复控制 COMMIT、缺失持久围栏、过早宣称结果确定和重复终态写入均被校验器拒绝。可用 `run_desktop.ps1 -RustVersion 1.99 -Scenario armed_child_fence_ack_loss` 复跑。

### 004 恢复查询超时

固定原 Java TC 登记原 AT 分支并保留锁。原 Rust 业务进程在 006 持久化后发出业务 COMMIT，代理截留请求，运行器终止原进程并独立确认其物理 MySQL 会话关闭。其后管理连接按该分支主键锁住见证为零的 004 行。恢复端使用真实生产查询，独立连接观察到该查询在同一个主键记录上等待管理连接。

MySQL 记录了准备语句执行失败，Rust 1.99 收到 `3024/HY000`，Rust 1.85 收到 `1205/HY000`。连接设置了两个等长的服务端限时，分别对应语句执行与 InnoDB 行锁等待，两次运行先触发的计时器不同。两次均由原始查询和服务端线程记录绑定同一个故障连接，Rust 都返回 `Observation` 未知。独立 SQL 在解除行锁前核对 TC 原分支和锁仍在，业务、Normal Undo、围栏、004 见证和 007 标记均为零。

解除行锁后，恢复端用新的 MySQL 连接重新查询同一原分支，持久化一次围栏，得到 `NotAppliedFenced`。原 TC 收到零条 BranchReport。原 TM 发起一次 GlobalRollback，替换 RM 只提交一次回滚终态。最终保留原围栏，TC 会话、分支和锁全部清空。

[Rust 1.99](run-armed_child_query_failure-1.99-3e863ec8/verified.json) · [Rust 1.85](run-armed_child_query_failure-1.85-c0057440/verified.json)

没有锁住 004 时，故障对照直接得到 `NotAppliedFenced`，触发了首次观察必须未知的断言。校验器还拒绝了六份证据变体，包括错误的服务端错误码、缺失的锁快照、提前写入的围栏、过早发布确定结果、替换失败查询的物理连接，以及在独立快照前复查。可用 `run_desktop.ps1 -RustVersion 1.99 -Scenario armed_child_query_failure` 复跑。

### 006 控制提交确认丢失

固定原 Java TC 为 Rust 业务子进程分配原分支。子进程在业务连接写入 Normal Undo 与候选见证，另用控制连接持久化 006。代理捕获这条控制 COMMIT 的原始 MySQL OK，在断开控制连接前保留响应并从独立连接查询。此时原 006、TC 分支和锁可见，业务行、Normal Undo、004 见证、007 标记和防悬挂记录均为零。

业务子进程真实收到 `Arming` 错误，内部为控制事务 `CommitOutcomeUnknown`。Rust 核对该错误携带原回执，并在原业务连接确认 ROLLBACK。报文覆盖这条业务连接的全部命令，业务 COMMIT 请求计数为零。子进程自行以状态零退出，独立 SQL 确认业务和控制物理会话均关闭，006 及业务状态与断连前快照一致。

随后，恢复端读取同一 006 持久记录，写入一次围栏并取得 `NotAppliedFenced`。原 TC 收到零分支报告。原 TM 发起一次 GlobalRollback，替换 RM 提交一次原分支回滚终态。最终保留唯一围栏，TC 会话、分支和锁全部清空。

[Rust 1.99](run-armed_child_arm_ack_loss-1.99-96d6c30d/verified.json) · [Rust 1.85](run-armed_child_arm_ack_loss-1.85-74de9f7a/verified.json)

正常转发 006 OK 的现场对照返回成功原回执，触发新用例对控制结果未知的断言。校验器还拒绝六份证据变体，包括业务 COMMIT 已发、006 OK 已交付、缺失业务回滚确认、错误的控制结果分类、缺失断连前持久 006、以及把独立快照移到业务回滚之后。可用 `run_desktop.ps1 -RustVersion 1.99 -Scenario armed_child_arm_ack_loss` 复跑。

### 006 控制提交请求未到达 MySQL

原 Java TC 登记真实分支后，Rust 业务进程在控制连接写入 006，并用同一原身份锁定检查。代理捕获该连接发出的 COMMIT 请求，截留请求和关闭控制连接之间通过另一 MySQL 会话查询。独立查询看到原 TC 分支和全局锁仍在，006、业务行、Undo 围栏和 004 见证均未持久化。

业务子进程收到带原回执的 `Arming` 错误，控制结果分类为 `CommitOutcomeUnknown`，原业务连接明确确认 ROLLBACK，业务 COMMIT 请求为零。子进程正常退出，两个原 MySQL 物理会话均消失。恢复端读取同一原分支时得到 `MissingAttempt`，不写围栏，成功报告入口也拒绝发送。

原 TM 发出一次 GlobalRollback 后，原 Java TC 通过替换 RM 回调原分支。原始 MySQL 报文确认同一物理事务插入一条围栏并推进回滚终态，仅一次 COMMIT 生效。最终 006 为空，原分支仅保留防悬挂记录，原 TC 的会话、分支和锁全部清空。

[Rust 1.99](run-armed_child_arm_blocked-1.99-08cedcaf/verified.json) · [Rust 1.85](run-armed_child_arm_blocked-1.85-071aab38/verified.json)

对照路径放行控制 COMMIT 并截留成功确认，结果实际持久化 006，触发了缺失尝试记录的断言。六种证据变体也被拒绝，包括转发控制 COMMIT、发出业务 COMMIT、独立快照中出现 006、把恢复结果改成 `NotAppliedFenced`、在回调前写围栏以及将终态写入移到另一物理事务。可用 `run_desktop.ps1 -RustVersion 1.99 -Scenario armed_child_arm_blocked` 复跑。

### 原业务会话持锁时并发恢复

固定原 Java TC 已登记分支并持锁，Rust 子进程已持久化 006，同时在原业务事务内持有候选见证和 Normal Undo。代理在原 COMMIT 发往 MySQL 前将其截留。独立 SQL 核对从另一连接观察的业务行、Normal Undo 和 004 见证均为零。此时启动另一进程中的第一次恢复。MySQL 记录的 `data_lock_waits` 显示，恢复连接正等待原业务连接在 004 表上的主键记录锁。原始请求参数将等待连接的成功准备查询绑定到原 XID 与 BranchId。

| 原业务事务的结果 | 同一次恢复与原 TC 终态 | 双工具链证据 |
| --- | --- | --- |
| 原进程终止，原 COMMIT 始终截留 | 原 MySQL 事务回滚后，同一次恢复持久化一次围栏并返回 `NotAppliedFenced`。原 TC 收到零报告，原分支回滚。 | [Rust 1.99](run-armed_child_wait_rollback-1.99-3135a247/verified.json) · [Rust 1.85](run-armed_child_wait_rollback-1.85-20e15ca1/verified.json) |
| 确认等待后放行原 COMMIT | MySQL 完成原提交，同一次恢复返回 `Applied`，没有写入围栏。代理截留业务 OK 并终止原进程，替换 RM 发送一次成功报告，原 TC 完成原分支提交。 | [Rust 1.99](run-armed_child_wait_commit-1.99-874b620f/verified.json) · [Rust 1.85](run-armed_child_wait_commit-1.85-cec59128/verified.json) |

两侧最终均清空 TC 会话、分支和锁。提交侧保留原业务数据，删除 Normal Undo。回滚侧保留唯一防悬挂记录。没有提前启动恢复的现场对照未观察到锁等待，运行器按预期失败。两份现场证据的校验器均拒绝替换阻塞者、替换等待连接、伪造首次结果、提前放行或写围栏，以及在释放前伪造业务行可见。复跑用例为 `armed_child_wait_rollback` 和 `armed_child_wait_commit`。

### 无变化本地提交与确认丢失

Rust 1.99 和 1.85 均用默认构建连接固定原 Java TC 与 MySQL 8.4.13。RM 服务先完成登记，TM 取得原 XID。业务侧在原 MySQL 事务里执行一条匹配零行的 UPDATE，经准备和仲裁确认无分支回执、Undo 或候选见证，再调用 `commit_no_change`。报文观察器核对实际 UPDATE、服务器零影响行与原连接的一次 COMMIT。

| MySQL COMMIT 响应 | 本地与全局结果 | 双工具链证据 |
| --- | --- | --- |
| 原始 OK 正常交付 | Rust 确认本地提交。原 TM 发出一次 GlobalCommit，原 TC 确认提交。 | [Rust 1.99](run-no_change_commit-1.99-2a0927e3/verified.json) · [Rust 1.85](run-no_change_commit-1.85-cd2156e1/verified.json) |
| 代理丢弃原始 OK 并关闭连接 | Rust 返回 `MySqlNoChangeCommitError::Unknown`。原 TM 发出一次 GlobalRollback，原 TC 确认回滚。业务 SQL 与本地 COMMIT 均未重发。 | [Rust 1.99](run-no_change_ack_loss-1.99-a6bef5d5/verified.json) · [Rust 1.85](run-no_change_ack_loss-1.85-f8d22511/verified.json) |

在原 TM 发包前，独立 SQL 确认原 XID 处于 Begin，分支、锁、Undo、004 恢复行和 006 尝试记录均为空。RM 原始帧只有服务登记及可能的心跳，零分支登记、分支报告或分支回调，分支日志无记录。全局结束响应与后续查询独立核对。原 TC 有时短暂返回 `AsyncCommitting` 或 `Rollbacking`，在有界等待内清理原 XID 后返回 `Finished`。业务表的原有行始终保持不变。

确认丢失用例在正常交付 OK 的现场对照中失败。校验器回放拒绝伪造 OK 已交付、重复本地 COMMIT、额外分支登记、预结束快照出现分支、错误的本地成功分类和实验特性构建。复跑用例为 `no_change_commit` 和 `no_change_ack_loss`。

### 无变化 COMMIT 超时后的连接池回收

Rust 1.99 和 1.85 均在默认构建、固定原 Java TC 和 MySQL 8.4.13 下使用同一个容量为 1 的业务连接池。原 UPDATE 匹配零行，业务连接只发送一次 COMMIT。代理收到原始 MySQL OK 后扣住响应，独立 SQL 此时仍能查到原物理会话。Rust 在本地提交期限到达后返回 `MySqlNoChangeCommitError::Unknown`，底层为 `DbError::Deadline`。

SQLx 在旧会话关闭时发出清理用的 ROLLBACK 和 COM_QUIT。原 COMMIT OK 早于这两个清理命令，且其状态位确认原 MySQL 事务已经结束。独立 SQL 查到原物理会话消失后放行后续操作。同一个业务池借出不同物理会话，开启新事务，执行一条不同的 UPDATE，真实服务器确认修改一行。探测事务显式回滚，独立 SQL 查到原有业务值仍为 `untouched`。

原 TM 随后按最初的 XID 完成一次 GlobalRollback，原 TC 清理会话并返回 `Finished`。RM 服务已登记，原始报文中零分支操作，分支、锁、Undo、004 和 006 最终均为空。

[Rust 1.99](run-no_change_pool_recovery-1.99-4ac56930/verified.json) · [Rust 1.85](run-no_change_pool_recovery-1.85-6b530e2f/verified.json)

正常交付 OK 的现场对照触发了超时分类断言。证据回放拒绝原会话仍在、缺失原 QUIT、清理早于原 COMMIT OK、复用旧物理连接、过早启动探测事务，以及缺失探测回滚确认六种变体。复跑用例为 `no_change_pool_recovery`。

### 有变化 COMMIT 超时后的连接池回收

Rust 1.99 和 1.85 在固定原 Java TC 和 MySQL 8.4.13 下使用容量为 2 的同一个业务池。原分支先在独立控制会话确认 006 持久化，随后在原业务连接提交业务、Normal Undo 和 004 见证。代理扣住这一次业务 COMMIT 的真实 OK，独立 SQL 确认原物理会话此时仍存在。客户端达到期限后返回带原成功回执的 `MySqlChangedCommitError::Unknown`，随后对旧事务发送清理命令并关闭原物理会话。

独立 SQL 确认旧会话消失后，同一池借出两条不同的新物理连接。第一条执行零行匹配的 UPDATE，并在第二条连接执行实际 UPDATE 与回滚确认期间一直持有租约，最后也确认回滚。原业务行保持 `original_tc_report`，Normal Undo 和 004 仍在，007 仍为零。运行器记录这些事实后才放行恢复。

恢复端锁定读取原 XID/BranchId 的见证并返回 `Applied`。007 门闩只更新一次并确认提交，原 RM 只发出一次 `PhaseOne_Done`，再次申请报告返回 `AlreadyStarted`。原 TM 发起一次 GlobalCommit，原 TC 对同一分支完成提交回调。最终保留原业务行和持久见证，Undo 及 TC 会话、分支、锁清空。原业务 SQL 与原业务 COMMIT 各执行一次。

[Rust 1.99](run-changed_pool_recovery-1.99-2a3008e5/verified.json) · [Rust 1.85](run-changed_pool_recovery-1.85-282b66ee/verified.json)

放行原业务 OK 的现场对照在首次结果断言处失败。证据回放拒绝伪造原 OK 已送达、重复原业务 COMMIT、把两次借用改为串行、移除 004 见证、提前发送分支报告和重复报告。复跑用例为 `changed_pool_recovery`。

### MySQL TLS 信任失败与原事务提交

固定原 Java TC 通过 Connector/J `VERIFY_IDENTITY` 使用新签发的夹具 CA 连接 MySQL 8.4.13。MySQL 强制 TCP 加密，独立查询记录了 Java TC 专用账号的四条 TLS 会话。Rust TM 先取得原 XID，RM 数据源使用无关 CA 时在 `Driver/connect` 被拒绝。MySQL 记录一次新增握手失败，独立 SQL 确认原 XID 仍处于 Begin，业务、Undo、分支、锁、004 和 006 均无记录。

运行器保存快照后才允许同一 Rust 入口换用正确 CA。原始报文和持久记录确认同一 XID 仅登记一个分支，报告一次 `PhaseOne_Done`，TM 发送一次 GlobalCommit，原 TC 完成一次对应分支回调。原业务行及 004、007 持久标记保留，Undo、TC 会话、分支和锁清空。

[Rust 1.99](run-tls_wrong_ca-1.99-b6e279ae/verified.json) · [Rust 1.85](run-tls_wrong_ca-1.85-ae9b5255/verified.json) · [错用受信 CA 的失败对照](run-tls_wrong_ca-1.99-4f2d37f6/client-stdout.log)

复跑命令为 `powershell -NoProfile -File tools/formal-first-phase-commit/run_desktop.ps1 -RustVersion 1.99 -Scenario tls_wrong_ca`。运行器使用现有 Python `cryptography` 和 JDK 17 `keytool` 生成材料。Docker 容器、私钥和信任库随本次运行清理。覆盖范围为 Java TC 与 Rust RM 各自到 MySQL 的服务端 TLS 身份验证。Seata 协议 TLS 和生产认证继续验收。

### 提交回调终态写入被 MySQL 拒绝

固定原 Java TC 与 MySQL 8.4.13 的现场夹具给 Rust RM 分配受限账号。原事务通过同一账号完成业务、Normal Undo、004 见证与一次 `PhaseOne_Done` 报告。该账号的列权限允许提交授权写入，暂缺 `commit_terminal` 的 UPDATE 权限。

首次回调内的真实 DELETE 匹配原 XID 和 BranchId，MySQL 返回影响一行。随后的终态 UPDATE 在同一物理连接的 Prepare 被 MySQL 以 1143／42000 拒绝。客户端向 MySQL 发送清理 ROLLBACK，原 TC 收到状态 6。独立 SQL 核对原 Undo 的 SHA-256 与回调前一致，业务行、004 与 007 保留，提交授权为 1，终态为 0。

只补授 `commit_terminal` 后，原 TC 重试同一分支。MySQL 记录原 Undo 删除、终态更新各影响一行，以及同一事务的 COMMIT OK。原始 RM 帧顺序为 6、5，TM 只发送一次 GlobalCommit。最终保留业务行，Undo 与 TC 会话、分支、锁清空。

[Rust 1.99](run-commit_permission-1.99-3cb5e21c/verified.json) · [Rust 1.85](run-commit_permission-1.85-bedb03cc/verified.json) · [提前授予列权限的失败对照](run-commit_permission-1.99-7515669d/expected-failure.log)

提前授予该列权限时，首次回调成功并触发测试的可重试状态断言。双工具链证据回放均拒绝缺失 MySQL 原权限错误、首次 DELETE 确认、清理 ROLLBACK 或重试 COMMIT 确认的变体。复跑用例为 `commit_permission`。

### Undo SHA-256 与同一分支恢复

013 显式迁移给 Rust 恢复表增加 nullable BINARY(32) 摘要。新登记版本为 4，仲裁阶段在原业务事务内从已验证的原 Undo 字节计算并保存 SHA-256，再写入见证。恢复及正式提交、回滚回调在使用新记录的 Undo 前校验。摘要不匹配使用独立错误类型，保持恢复未知及回调可重试。原 Java Undo 表和序列化字节格式保持原定义，版本 1 至 3 行沿原语义读取。

固定原 TC 与 MySQL 8.4.13 联试先修改原 Undo 的等长镜像值，独立恢复拒绝。恢复原字节后按原身份得到 Applied 并报告。随后再次修改 Undo，原提交回调返回状态 6；独立 SQL 核对摘要仍对应原字节且当前 Undo 摘要不同，才恢复原字节。原 TC 重试同一分支并完成提交。两版本库测试各 41 项通过、4 项按原配置忽略，严格 Clippy 通过。

[Rust 1.99](run-undo_checksum-1.99-5441f8de/verified.json) · [Rust 1.85](run-undo_checksum-1.85-10cd49be/verified.json)

复跑场景为 undo_checksum。首次回调试验发现 RecoveryConflict 被映射为不可重试状态 7，随后使用 UndoChecksumMismatch 修正，以上通过记录均验证状态 6 后恢复原字节的路径。

### Undo 摘要的回滚与迁移兼容

原 TM 在摘要被破坏后发送一次 GlobalRollback。原 TC 第一次收到状态 9，恢复原字节后重试同一 XID、BranchId 并收到状态 8。最终业务行被补偿删除，保留一个 GlobalFinished 防悬挂记录，TC 会话、分支和锁清零。两套工具链的分支日志均只有 Pending、Completed 两个持久状态。

| Rust | 摘要破坏后的回滚 | 旧行应用 013 后的恢复提交 |
| --- | --- | --- |
| 1.99 | [结果](run-undo_checksum_rollback-1.99-6ad131d7/verified.json) | [结果](run-undo_checksum_legacy-1.99-fcf0f703/verified.json) |
| 1.85 | [结果](run-undo_checksum_rollback-1.85-5e9f254f/verified.json) | [结果](run-undo_checksum_legacy-1.85-87ebaa0c/verified.json) |

迁移用例先使用原物理表提交真实业务和 Undo，再执行 013，SQLx 读取确认版本 3 和 NULL 摘要。原身份恢复为 Applied 后，原 TC 完成同一分支提交。复跑场景为 undo_checksum_rollback 与 undo_checksum_legacy。双工具链严格 Clippy、格式、Python AST 和 PowerShell 语法检查通过。

### 摘要异常时的恢复报告拒绝

新版本 4 分支的摘要被设为 NULL 时，直接调用 report_recovered_applied 返回 UndoChecksumMismatch。恢复摘要后再改写 Undo，该报告入口同样拒绝。两次拒绝后 007 门闩仍为零。恢复原字节后，原始帧总计仅一次 PhaseOne_Done，随后完成原 TC 提交。Rust 1.99 与 1.85 的新记录均保留 missing_checksum_report_rejected、modified_undo_report_rejected 字段。

[Rust 1.99](run-undo_checksum-1.99-23edf93a/verified.json) · [Rust 1.85](run-undo_checksum-1.85-d11d60ef/verified.json)
