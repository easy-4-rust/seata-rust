# TM Launcher 008 写入与发送门禁

2026-11-25。在正式 `seata-tm` 中增加独立 `begin_durable` 路径，原普通 `begin` 保持既有协议行为。`LauncherOrigin` 由实际成功 Begin 的 XID、真实连接池端点、登记应用/服务组、原异常码、可选附加数据和业务意图生成；MySQL 控制连接在交付句柄前插入 008 行。期限使用 Begin 后剩余时间再扣除控制操作预算，避免数据库提交延迟把恢复期限推后。

持久句柄的 Commit/Rollback 先 CAS 写入原操作，再 CAS 将 `send_started` 从 0 改为 1；任一步未确认即不发送。写后网络未知错误撤销旧内存 `recover_unknown` 凭证，避免绕开持久门闩。此路径不提供跨进程 Launcher 接管。

先运行 `cargo test -p seata-tm --test durable_launcher --offline`，因缺 `LauncherOrigin`/`LauncherStore` 而编译失败。实现后 3/3 通过：来源写入被拒时不返回 Launcher；发送标记被拒时真实 TCP 对端未收到 Commit；Commit 帧已到但响应丢失时，旧内存恢复入口拒绝该错误。Rust 1.99/1.85 的目标严格 Clippy `-D warnings` 和 Rust 1.99 全仓格式检查均退出 0。TM 全套测试退出 0，MySQL 单元测试 40 通过、4 个需现场 MySQL 的测试按标记跳过。

`tools/formal-first-phase-commit/run_tm_launcher_store.ps1` 在随机 `undo_tm_*` 库、自有回环 MySQL 8.4.13 容器与受控 TCP 对端运行 `live_tm_launcher_store`。Rust 1.99 与 1.85 各通过 1/1：原 XID、端点、应用、服务组、Begin 附加字段及业务意图逐项匹配；收到 GlobalCommit 帧时，独立数据库连接已看到 `end_operation=1,send_started=1`。测试失败的首次运行由测试夹具在 TM 关闭前等待 EOF 导致，调整夹具关闭顺序后双版本通过。容器运行后已删除，随机密码未保存。

## 结束帧故障补验

新增独立子进程夹具：`ExitAfterMarkerStore` 委托真实 MySQL 持久层提交 `send_started=1` 后直接退出进程。父进程受控 TCP 对端确认没有收到 Commit 字节；独立 MySQL 查询看到原 XID、`end_operation=1`、`send_started=1`、`owner_epoch=1`。Rust 1.99/1.85 的 `marker_exit_retains_one_way_send_claim` 各通过 1/1。Windows 在进程直接退出时可能返回 EOF 或 `ConnectionReset`，夹具接受这两种无帧断开。另一个受控对端用例接收 Commit 后丢弃回包，双版本各通过 1/1，门闩保持 1，旧内存恢复凭证被拒绝。以上用例用 `run_tm_launcher_store.ps1 -TestName ...` 在每次全新随机库运行。

固定原 Java TC 的进一步联试由 `run_desktop.ps1` 的 `tm_durable_commit_loss`、`tm_durable_rollback_loss` 两个场景启动。原 Java pin、JAR/Connector 哈希、认证覆盖层和受标签的 MySQL 8.4.13 镜像仍由既有 runner 验证。最终 008→009 迁移路径下 Rust 1.99/1.85 的 [Commit 1.99 结果](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.99-b0814117/verified.json)、[Commit 1.85 结果](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.85-b50124c8/verified.json)、[Rollback 1.99 结果](../java-tc-changed-commit-report/run-tm_durable_rollback_loss-1.99-c9ff26ca/verified.json)、[Rollback 1.85 结果](../java-tc-changed-commit-report/run-tm_durable_rollback_loss-1.85-966275d1/verified.json)各通过 1/1：原 TC 收到一次真实结束帧、返回同请求 ID 成功响应，TM 帧代理只截留该响应；Rust 将结果保留为未知并拒绝旧内存恢复入口，持久门闩为 1，操作分别为 Commit=1 或 Rollback=2。四次 `TM-frames.jsonl`、`process-cleanup.json` 均在对应证据目录；原始帧代理未观察到第二次结束请求。两工具链严格目标 Clippy、全仓格式和 Python AST 检查退出 0，自有容器已删除。

此证据覆盖真实原 TC 的 Commit/Rollback 成功回包丢失，以及受控对端的进程退出点；还没有原 Java TC 下的 TM 跨进程接管、其他结束帧故障状态、旧拥有者停止认证或生产权限配置。008 行本身不授予恢复权，WP05/WP06 整体门禁未关闭。

## 恢复前只读来源核验

正式 MySQL 持久层新增 `load_snapshot(xid)`：先按 SHA-256 摘要定位，再比较完整原 XID；只返回来源身份、原 Begin 字段、业务意图、操作/发送状态、代际与 MySQL 服务端时钟的期限判断，绝不授予 Launcher 或 CAS 权限。`snapshot_rejects_same_digest_with_different_full_xid` 先在缺 API 时编译失败；实现后在 Rust 1.99/1.85 各用真实 MySQL 通过 1/1：不存在的摘要为 `None`，同摘要但不同原 XID 的伪造行被拒为冲突。真实 Begin 路径的 `begin_source_and_end_markers_precede_real_commit_frame` 双版本复跑各通过 1/1：快照保留原端点、登记身份、业务意图；Commit 后观察到操作和门闩，数据库期限改为过去后由服务端判断已过期。读取能力仍不能代替旧拥有者停止证明或 TC 状态查询。

## 009 实例绑定与历史行迁移

009 以允许 `NULL` 的 `BINARY(32)` 列增量保存 TM 管理器实例 ID。正式 Begin 从操作系统随机源签发 ID，持久来源、只读快照和两次 CAS 均核对该 ID。`legacy_008_row_remains_unowned_after_009_migration` 在真实 008 表插入旧行后运行 009，确认旧 ID 保持 `NULL`，新 CAS 影响零行，快照返回冲突。`changed_owner_instance_blocks_original_end_cas` 改写来源中的 ID 后确认原管理器无法结束，也没有发出结束帧。独立子进程在标记写入后立即退出时，父进程核对子进程报告的 ID 与数据库 ID 一致，并确认对端无结束帧；Rust 1.99/1.85 均通过。这些测试建立实例关联，尚未证明跨主机的旧进程停止认证。

`marker_ack_loss_after_persisted_cas_never_emits_end_frame` 用委托真实 MySQL 写入的测试存储层模拟确认回包丢失：数据库门闩已为 1，但 TM 得到错误，受控 TCP 对端没有结束帧；双工具链均通过。该用例模拟写后确认丢失，不是 MySQL 线上 OK 包丢失的注入证明。最终代码的 Rust 1.99/1.85 严格 Clippy、Rust 1.99 格式检查，以及相关 TM/MySQL 单元测试通过。009 的实例 ID 只能绑定原签发者，不能单独授予恢复权限；可信停机证明与生产数据库权限仍待实现。

## MySQL Launcher 角色验收

同一真实 MySQL 8.4.13 测试应用 008、009 与 [角色 SQL](../../../../crates/seata_db_mysql/sql/tm_launcher_roles.sql)，为两个账号分别启用写入和观察默认角色。Rust 1.99 与 1.85 的 `begin_source_and_end_markers_precede_real_commit_frame` 各通过 1/1。正式 TM 用写账号完成 Begin 和 Commit，独立观察账号读取原 XID、实例 ID、意图、发送标记和数据库时钟判定的期限。观察账号的 INSERT、UPDATE、DELETE 收到权限拒绝。写账号也被禁止删除来源、更新实例 ID 或代际以及在插入时指定代际或发送标记。收到真实 TCP Commit 帧时，独立数据库连接已看到意图和发送标记。写账号属于可信运行时，单向状态转换仍由正式存储层的 CAS 保证。

## 新 TM 管理器查询原 TC

Rust 1.99/1.85 的真实 MySQL 与受控 TCP 用例关闭原管理器，再由具有独立实例 ID 的管理器查询同一 XID。端点、应用、服务组和业务意图不匹配以及快照读取时已过期的调用均被拒绝。TCP 对端仅接收一次状态请求，返回 `Finished`，客户端保留该状态。缺少两个查询接口时目标测试编译失败，加入实现后双版本通过。Rust 1.99 TM 离线回归 35 项通过，两版本目标严格 Clippy 与全仓格式检查通过。

固定原 Java TC 的 Commit/Rollback 回包丢失场景也改用新管理器查询来源。两种操作在 Rust 1.99 和 1.85 均通过，分别观察到 `AsyncCommitting` 和 `Rollbacking`。帧观察器确认每次只有一个结束请求和一个状态查询，查询在新连接上发送，查询 XID 与结束请求逐字节一致。状态回包与查询的连接及请求 ID 匹配，原始状态码与 Rust 返回值相同，数据库发送标记保持 1。首次运行暴露了立即查询可能仍处于处理中，测试随后保留原状态，并包含处理中与已结束的预期。证据见 [Commit 1.99](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.99-b33df5d3/verified.json)、[Rollback 1.99](../java-tc-changed-commit-report/run-tm_durable_rollback_loss-1.99-6b184a7e/verified.json)、[Commit 1.85](../java-tc-changed-commit-report/run-tm_durable_commit_loss-1.85-4aa3d1c2/verified.json)、[Rollback 1.85](../java-tc-changed-commit-report/run-tm_durable_rollback_loss-1.85-9bd61dbb/verified.json)。此处替换 TM 管理器和连接，可信进程停机证明与跨进程接管仍待验收。

## 本机进程退出证明

正式 `LauncherProcessMonitor` 持有自己启动的子进程句柄，从该子进程的输出管道读取实例报告。真实 MySQL 8.4.13 用例 `marker_exit_retains_one_way_send_claim` 在收到 Begin 帧后暂扣回包，此时工作进程已报告 ID 且仍在运行，监护器返回无证明。放行后，子进程确认发送门闩写入即退出。原监护器核对 OS 退出状态，生成与数据库来源一致的实例证明，受控 TC 对端收到零结束帧。其他实例、另一监护器和未报告 ID 的退出进程均无法通过核验。Rust 1.99 与 1.85 各通过 1/1，双版本目标严格 Clippy、Rust 1.99 格式检查及 TM 35 项离线回归通过。证明只在保留原监护器的受信控制进程中使用，代际更新和跨主机认证继续开放。

## 本机代际更新

`supervised_exit_allows_only_one_owner_transfer` 先在缺少正式 API 与角色 SQL 时编译失败。实现后，在 Rust 1.99/1.85 的真实 MySQL 8.4.13 和受控 TCP 联试中，旧子进程在提交 Commit 意图后退出，独立恢复账号以实际停止证明申请代际。另一监护器被拒，`Finished` 以原状态返回；测试在收到查询后改为过期，即使 TC 返回 `Begin`，数据库也拒绝更新。恢复夹具期限后，两次并发申请恰好一次从 1 更新到 2，落盘新 TM 的实例 ID，Commit 意图保持 1，发送门闩保持 0。测试还确认恢复账号无法写发送门闩，TCP 对端只见状态查询。两工具链目标严格 Clippy 和格式检查通过。待验为新代际的单次发送、跨主机停止认证与固定原 Java TC 下的独立进程恢复。

## 新代际单次发送

测试先因代际申请只返回数字、缺少结束句柄而编译失败。正式实现把已确认的代际交给不可复制的 `MySqlTransferredLauncher`，用匹配的管理器权限及同库写账号消耗句柄。Rust 1.99/1.85 在真实 MySQL 8.4.13 与受控 TCP 上各通过 Commit 成功与 Rollback 回包丢失两例。原管理器所属子进程在持久化原意图后退出，新管理器先经受信代际申请，再查询原 TC 并确认门闩。对端见到一次原结束帧时，独立查询已可见代际 2、原操作值、`send_started=1`。Rollback 丢回包时结果保留未知，同进程恢复凭证与重复代际申请均被拒。另有跨管理器权限拒绝测试，TM 离线回归 36 项通过，双版本严格 Clippy 与格式检查通过。固定原 Java TC 的独立进程恢复与跨主机停止认证继续开放。

## 固定 Java TC 下的独立 TM 进程接管

使用受校验的 Java pin、原 JAR 和 Connector/J、MySQL 8.4.13，新增 `tm_recovered_commit` 与 `tm_recovered_rollback_loss`。旧 TM 子进程在真实 Begin 与结束意图落盘后退出，监护器核验退出证明，父进程的新 TM 申请代际并消耗一次性发送句柄。每次联试由观测器验证两个不同 PID、一个原 Begin、新连接上对同一 XID 的两次 `Begin` 状态回包及一个原结束请求。Commit 的原 TC 回包送达，TM 观察到 `Committed`。Rollback 的成功回包被截留，TM 保留未知，拒绝旧内存恢复。独立数据库查询确认持久代际为 2、发送标记为 1，协调器的 global/branch/lock 行数均为 0。

四组均通过，帧、数据库结果和清理记录见 [Commit 1.99](../java-tc-changed-commit-report/run-tm_recovered_commit-1.99-c8ae9ee7/verified.json)、[Rollback 1.99](../java-tc-changed-commit-report/run-tm_recovered_rollback_loss-1.99-76d3a3c9/verified.json)、[Commit 1.85](../java-tc-changed-commit-report/run-tm_recovered_commit-1.85-c6786a19/verified.json)、[Rollback 1.85](../java-tc-changed-commit-report/run-tm_recovered_rollback_loss-1.85-737b4823/verified.json)。首次运行暴露测试框架输出与实例报告占用同一行的问题，移除子进程的串行测试输出选项后通过。观察器只在监督场景接受旧连接上的单次 Windows 退出重置。双工具链目标严格 Clippy、格式检查与 Python 语法检查通过。此处使用无业务分支的全局事务，AT 数据与 Undo 的最终状态和跨主机停止认证仍待独立验收。

## 固定 Java TC 下带真实 AT 分支的接管

`tm_recovered_at_commit` 和 `tm_recovered_at_rollback_loss` 使用 `experimental_changed_commit` 配置。旧 TM 与 RM 在子进程内完成一阶段业务写入、Normal Undo 和一次 `PhaseOneDone` 报告，随后旧 TM 持久化结束意图并退出。结束前的数据库采样核对业务行、同一分支的 Normal Undo 和 TC 全局锁各一条。新的 TM/RM 进程从同一 MySQL 来源接管，原始帧核对固定 TC 只收到一次结束请求、只向新的 RM 连接发出一次匹配原 XID 和 BranchId 的 AT 回调。独立查询核对 TC 的 global/branch/lock 均清零，分支日志进入完成状态。

- Commit 保留一行业务数据，清除 Undo，分支进入提交终态。
- Rollback 成功回包被 TM 代理截留，TM 保留未知且拒绝重发。业务行与 Normal Undo 被清除，原 XID 和 BranchId 留下一条 `GlobalFinished` 围栏。首次现场测试误将该围栏判为残留 Undo，之后分别断言 Normal Undo 为零及围栏的身份和内容，并以 MySQL 1062 验证晚到的同键 Normal 写入被拒绝。

Rust 1.99 和 1.85 四组现场均通过，分别见 [Commit 1.99](../java-tc-changed-commit-report/run-tm_recovered_at_commit-1.99-8c5ea300/verified.json)、[Rollback 1.99](../java-tc-changed-commit-report/run-tm_recovered_at_rollback_loss-1.99-a1150fed/verified.json)、[Commit 1.85](../java-tc-changed-commit-report/run-tm_recovered_at_commit-1.85-73f2d5a8/verified.json)、[Rollback 1.85](../java-tc-changed-commit-report/run-tm_recovered_at_rollback_loss-1.85-aa1ad18a/verified.json)。各目录保存 TM/RM 原始帧、独立 SQL `at-terminal.tsv` 与清理记录。两版本目标严格 Clippy、格式检查、无特性编译与 Python 语法检查通过。跨主机认证和 V1 全部门禁继续开放。

## 可传输的停止签名

签发方使用真实退出的 TM 子进程及原进程监护器生成 Ed25519 签名，接管方使用预配公钥、信任域和限时挑战核验后进入真实 MySQL 8.4.13 代际 CAS。测试初次因缺少签名与接管 API 编译失败，加入实现后在 Rust 1.99 和 1.85 都通过签名接管 Commit 与 Rollback 回包丢失用例。验证拒绝另一监护器签发、字节篡改、非预配密钥、错误信任域、不同挑战、接管实例不匹配及过期挑战。有效签名的两个并发申请只提交一个新代际，发送门闩仍限制原操作只发一次。双工具链严格目标 Clippy 和全仓格式检查通过。签发与验证在本机夹具中调用；独立主机上的传输、密钥配置及部署验收仍待完成。

## 停止签名交换

新增 `LauncherStopExchange`，每条流传输一次 68 字节请求与 68 字节应答。旧监护器先核对请求的原实例及退出证明，接管方按预配公钥核验收到的签名。测试先因缺少交换 API 编译失败。实现后，独立签发子进程通过真实回环 TCP 返回证明，接管进程使用预先加载的公钥核验，且签发进程不回应错误实例。另验证错误版本、截断、正确帧头下的伪造签名和无响应超时。Rust 1.99 和 1.85 的 TM 离线回归各通过 38 项；2 个子进程入口按标记由父测试启动。真实 MySQL 的 Commit 与 Rollback 回包丢失场景使用双工流交换签名再申请代际，两个版本均通过，单次发包约束保持。两版本严格 Clippy、全仓格式检查通过，测试环境已清理。部署仍需为字节流配置对端认证及主机公钥，并在两台独立主机上验收。

## 停止证明双向 TLS

新增 `LauncherStopTlsClient` 与 `LauncherStopTlsServer`。真实回环 TCP 先完成 TLS 1.3 双向证书验证、服务端主机名与专用 ALPN 核验，再比对双方叶证书的预配 SHA-256 指纹，最后交换停止签名。测试使用运行时生成的临时 CA 与证书，初次因缺少适配器编译失败；实现后两个工具链均通过正确证书、同 CA 未登记客户端证书、错误服务端指纹、错误主机名、无客户端证书和停滞握手用例。Rust 1.99/1.85 的 TM 离线回归各 39 项通过，目标严格 Clippy 和全仓格式检查通过。共享离线缓存补齐新依赖后，固定 Java TC 的 [Rust 1.85 AT Commit 接管回归](../java-tc-changed-commit-report/run-tm_recovered_at_commit-1.85-71662cb0/verified.json)也通过。该回归沿用本机接管，TLS 本次在独立回环夹具中核验。跨主机证书发放、密钥轮换和部署验收仍待完成。

## 停止签名公钥轮换

轮换权限与验证器分开，部署控制方可原子安装新旧两个公钥，并在签发方切换后把清单缩减为新公钥。真实回环双向 TLS 用例使用轮换前创建的挑战，验证重叠期两个签名都可用，退役后旧签名被拒而新签名仍可用。

真实 MySQL 负例在原 TC 收到状态查询但尚未回包时退役旧公钥。首次运行实际未拦住该代际申请；加入行锁后的签名与挑战期限复验后，旧申请返回冲突，代际保持 1。后续用新签名发起两个并发申请，只确认一个新代际并发送一次原结束请求。Rust 1.99/1.85 各通过 Commit 和 Rollback 回包丢失两例。双版本 TM 各 39 项离线回归、目标严格 Clippy 和格式检查均通过。多主机证书和公钥配置分发以及双主机部署验收继续开放。

## 持久化停止签名信任域

010 迁移增加可空的 `owner_stop_scope`，保留 008/009 历史来源的归属。TM 创建来源时记录配置的信任域，远程接管验证来源的域与验证器一致，并在行锁下比较原域。成功接管随新代际保存接手方的域。未绑定信任域的来源无法凭远程证明接管。

Rust 1.99 和 1.85 在真实 MySQL 8.4.13 上各通过四项测试，覆盖 Commit、Rollback 回包丢失、008/009 升级及真实提交帧前持久化来源和发送标记。接管测试拒绝由同一可信密钥为另一域生成的有效签名、未绑定域的来源和失效快照，并确认只有一个并发接管写入新代际。新建最小权限写账号可插入域字段，但无法改写它。两个版本的目标严格 Clippy、格式检查和联试工具 Python 语法检查通过。[固定 Java TC 联试](../tm-stop-nodes/README.md#010-信任域绑定回归)。

### 既有账号增量授权

Rust 1.99 和 1.85 各在独立 MySQL 8.4.13 容器中复原 009 的写、读和接管角色，并以原写账号创建一条来源。执行 010 后补授权前，MySQL 拒绝写账号插入域、接管账号更新域。连续两次执行生产授权脚本均成功，既有连接随后可分别插入和更新该列。写账号仍无法更新域，接管账号无法插入来源或更新发送门闩，读账号的插入和接管更新仍被拒绝。读账号查到原来源的域仍为 `NULL`，新来源的拥有者及域随代际更新。两个容器均已移除。[Rust 1.99 结果](run-010-grants-880e30383b78/1.99.0/test.log) · [Rust 1.85 结果](run-010-grants-880e30383b78/1.85.0/test.log)。
