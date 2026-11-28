# Seata-Rust V1 能力矩阵与恢复操作指南（预发布，2026-11-26）

本文件按当前工作树和[原 V1 验收清单](../phase-0/v1-implementation-acceptance-checklist.md)列出可核验能力及故障时的安全动作。**尚无可声明为生产可用的完整 V1 配置**：WP05、WP06、WP07 和完整 WP00-R 仍开放。表中的“已验证”只适用于对应证据所写的原 Java TC 版本、工具链和测试环境。

回调响应现有[原代际本地写入事件](evidence/rm-callback-write-events/README.md)，可关联原请求和决策；本地写入成功仍不能证明 TC 收到。

## 能力矩阵

| 能力 | 当前状态 | 已有证据与实际边界 |
|---|---|---|
| 原生 Java V1 消息与帧 | 初期 peer profile 已验证 | [WP01](../phase-1/evidence/protocol-peer-profile/README.md)覆盖批准的类型和显式 Go 适配；原 Go 默认路径差异、完整 WP00-R 与外部 TC 风险未结。 |
| TCP 连接、登记、请求关联与回包 | 初期 peer profile 已验证 | [WP02](../phase-1/evidence/transport-recovery/README.md)含原 TC 响应丢失；生产 TLS/认证策略和端点发现仍开放。 |
| Seata 传输 TLS 1.3 / mTLS | 显式选择的代理终止 profile 已验证 | 原生连接、登记池及 TM/RM builder 支持双向认证且不回退；[双版本原 TC 证据](evidence/transport-tls/README.md)核对重连、回包丢失及写出 flush。代理后端只限同机回环，生产证书/身份授权与部署门禁开放；不声称 Java TC 原生 TLS。 |
| Rust TM Begin/Commit/Rollback、传播、同进程未知结果恢复 | 初期 TM profile 已验证 | [WP03](../phase-1/evidence/tm-gate/README.md)通过原 Java TC；`recover_unknown` 只接受原管理器的内存凭证，不能作 TM 跨进程 Launcher 接管。 |
| Rust RM 登记、分支报告、回调与持久状态 | 初期 RM profile 已验证 | [WP04](../phase-1/evidence/rm-gate/README.md)验证重复与重连；其资源动作夹具不证明 MySQL Undo 原子性。 |
| MySQL AT SQL/Undo 准备及真实数据库局部行为 | 实施与验证中 | [WP05 各增量](../phase-0/v1-implementation-acceptance-checklist.md#wp-05--mysql-at-phase-1)包含真实 MySQL 与固定 TC 选定场景；完整 SQL 支持矩阵、外部写入器锁覆盖和一阶段故障门禁未结。 |
| MySQL 原生 JSON 非主键列 | 选定 JDBC / MySQL 配置已验证 | 元数据、精确服务器文本镜像、原 Java Undo 及原 TC 补偿已通过双 Rust 现场；SQL NULL 与 JSON null 分离，原生类型和字段预算严格核对。[JSON 证据](evidence/mysql-json/README.md)。SQL 语法和生产提交准入不因类型扩展而放开。 |
| 无变化、无分支本地 COMMIT | 受限实现 | `MySqlCommitCheckedTransaction::commit_no_change` 仅允许无登记、无 Undo、无见证作用域；成功不证明有变化业务的 Undo 原子提交。见[提交契约](mysql-first-phase-commit-contract.md)。 |
| 有变化本地 COMMIT、原报告和跨进程 RM 恢复 | **默认关闭的实验特性** | `experimental_changed_commit` 下的 006 尝试记录、004 见证、007 报告门闩有[选定故障证据](evidence/mysql-commit-attempt-arm/README.md)。两个真正独立恢复者进程竞争已在[受控 RM 对端](evidence/mysql-report-process-race/README.md)与[固定原 Java TC](evidence/java-tc-changed-commit-report/README.md)的真实 MySQL 场景双工具链通过；持久 TM 接管已有[停止证明与原 TC 联试](evidence/tm-stop-nodes/README.md)，完整外部写入器准入和生产部署继续验收。 |
| Undo SHA-256 | 013 迁移的新版本行已验证 | 版本 4 的摘要与业务、Undo、见证同事务提交，独立恢复和正式回调拒绝改写。旧行保留 NULL 摘要及历史语义。[双工具链现场证据](evidence/java-tc-changed-commit-report/README.md#undo-sha-256-与同一分支恢复)。 |
| MySQL AT 二阶段补偿与幂等 | 选定实验配置十项已验证 | [WP06 验收项](../phase-0/v1-implementation-acceptance-checklist.md#wp-06--mysql-at-phase-2)包含原 TC、MySQL、幂等、回包丢失和断连证据。生产验收仍依赖 WP05 完整门槛。 |
| 独立 Undo API 调用指标 | 选定 MySQL 场景已验证 | 固定 20 条只读序列区分完成、未知、提交前错误和取消；重复围栏与明确观察的恢复行数不代表全部实际效果。[指标证据](evidence/mysql-undo-metrics/README.md)。生产采集/告警仍开放。 |
| 生产可观测、部署配置、TLS/认证、负载与安全 | 未验收 | [WP07](../phase-0/v1-implementation-acceptance-checklist.md#wp-07--production-readiness)仍开放；[依赖审计](evidence/wp07-dependency-audit/README.md)仍报告 `rsa`。 |
| 其他 AT 数据库、XA、TCC、Saga、Rust TC | V1 范围外 | 按原[能力决策](../phase-0/v1-implementation-acceptance-checklist.md#5-v1-supported-feature-matrix)分别立项，不通过 MySQL AT 证据推断支持。 |

## 当前实验环境的恢复决策

恢复入口只接受**原 TC 分配的 XID、BranchId 与同资源 MySQL 管理器**。006 保存原成功登记身份但不签发新的 `RegisteredBranch`；004 保存本地一阶段见证；013 为新版本 4 恢复行保存原 Undo 字节的 SHA-256；007 的 `report_started=1` 仅表示报告**可能**已经发出。恢复不会授权重放业务 SQL、重新 BranchRegister 或由 RM 替 TM 发起全局结束。

```mermaid
flowchart TD
    Start[原分支结果未知] --> Record{006 原尝试记录存在且身份匹配?}
    Record -->|否/读取失败| Unknown[保持 Unknown 并保留证据]
    Record -->|是| Observe{004 原身份、见证与 Undo 摘要一致?}
    Observe -->|已提交| Applied[Applied: 本地一阶段已提交]
    Observe -->|缺行/冲突/查询失败| Unknown
    Observe -->|见证为零| Fence[按原 Undo 唯一键持久围栏并重新核对]
    Fence -->|Normal Undo 胜出且事实一致| Applied
    Fence -->|guard 确认且见证仍零| NotApplied[NotAppliedFenced: 本地未提交]
    Fence -->|期限/失败/回包丢失| Unknown
    Applied --> Claim{007 门闩 0→1 已持久确认?}
    Claim -->|是| Report[使用原 RM 客户端单次 PhaseOne_Done]
    Claim -->|否或未知| NoReport[不得发送或重发报告]
    NotApplied --> NoReplay[不得重放业务或重新登记]
```

| 可观察结果 | 操作动作 | 不可据此推断 |
|---|---|---|
| `MySqlCommitRecoveryOutcome::Applied` | 本地提交已确认；仅通过 `report_recovered_applied` 争抢 007 并报告，保留调用结果与 TC 帧。 | 不等于全局已 Commit、TC 已收到报告或二阶段已完成。 |
| `NotAppliedFenced` | 保留原 XID/BranchId、guard 与 004 证据；交由原 TC 的超时/决策路径处理。 | 不授权重做业务 SQL、重新申请分支或手工删 Undo guard。 |
| `MySqlCommitRecoveryError`、缺 006/004、查询或围栏结果未知 | 停止自动动作，保存错误和相关数据库事实；在恢复环境正常后可**重新观察原身份**。 | 见证零或缺行不能证明 COMMIT 未送达。 |
| `MySqlRecoveredReportError::AlreadyStarted` 或 `Claim` | 禁止再次发出报告；查询原 TC 状态和审阅 007/帧记录。 | `report_started=1` 不能证明 TC 收到报告。 |
| 原报告调用错误或响应丢失 | 保留原错误与已确认的本地事实；不自动重报。 | 不能把超时归类为 TC 明确拒绝。 |
| `DbError::UndoChecksumMismatch` | 保留原 XID、BranchId、版本、持久摘要和当前 Undo 证据，保持未知或让原 TC 重试。先处理存储完整性问题，再按原身份重新观察。 | 不授权报告成功、重放业务或用当前 Undo 重新计算并覆盖持久摘要。 |
| TM 结束结果未知 | 原管理器用原错误凭证查询。跨进程接管须读取持久 Launcher 来源、核验原所有者停止证明，再按 owner epoch 争抢结束权。使用已发送的结束标记时只观察原 XID。[停止证明与接管契约](evidence/tm-stop-nodes/README.md)。 | `Finished` 不证明历史 Commit，XID 也不是 Launcher 凭证。 |

操作时先冻结对应写入器的**新业务流量**，保存原始 RM/TM 帧、TC 状态、错误类别及原 XID/BranchId，然后只读核对数据库。对 006/004/007 的检查应使用绑定参数和只读权限；不要把凭据或原 `application_data` 写入日志。下列查询是核对字段的模板，`?` 表示由客户端绑定的原 XID 与 `branch_id`：006/004 的 VARBINARY XID 绑定 UTF-8 原字节，原 `undo_log` 的 VARCHAR XID 绑定原字符串；它不是可直接粘贴到 `mysql` CLI 的文本占位符：

```sql
SELECT HEX(xid), branch_id, HEX(resource_id), attempt_state, report_started
FROM seata_rs_at_commit_attempt
WHERE xid = ? AND branch_id = ?;

SELECT HEX(xid), branch_id, first_phase_committed,
       commit_authorized, commit_terminal
FROM seata_rs_at_branch_v1
WHERE xid = ? AND branch_id = ?;

SELECT xid, branch_id, log_status
FROM undo_log
WHERE xid = ? AND branch_id = ?;
```

应用 013 的环境还可查询下面的原分支摘要。旧表尚无此列，先确认迁移状态再执行。版本 1 至 3 的 NULL 摘要表示历史记录未承诺该完整性检查；版本 4 已提交且未终结的分支若摘要缺失，恢复保持未知。

```sql
SELECT schema_version, HEX(undo_sha256)
FROM seata_rs_at_branch_v1
WHERE xid = ? AND branch_id = ?;
```

只读检查不能替代 `recover_commit_attempt` 的原 Undo 唯一键围栏、身份校验和二次见证读取。不得手工更新 004/006/007、`undo_log` 或原 TC 锁表以制造预期状态。升级实验写入器前应停止并排空旧版本，按 001→007 顺序应用 AT 基础迁移，启用新摘要写入器前再显式应用 013；005/011 绑定、012 受信约束过程以及 008→010 TM Launcher 迁移按对应功能部署，详见[迁移目录](../../crates/seata_db_mysql/migrations)，核验目标 schema、InnoDB 与资源绑定；007 对旧 006 行默认置 1，故旧行不能被当作“尚未报告”。连接数据库时，`MySqlOptions` 默认 TLS VerifyIdentity；关闭 TLS 仅为显式字面 loopback 测试选项。固定原 Java TC 与 Rust 到 MySQL 的强制 TLS 和错误 CA 已有[现场联试](evidence/java-tc-changed-commit-report/README.md#mysql-tls-信任失败与原事务提交)，Seata 协议 TLS 及生产配置继续验收。

## 上线前仍需证明

WP05 的公开有变化 COMMIT、完整写入器准入、来源案例完整迁移，以及 WP06 对生产配置的最终验收。所测配置已验证提交/补偿、重复、响应丢失和持久 TM 接管，完整部署门槛继续开放；WP07 的可观测性、关闭策略、TLS/认证、负载与故障注入、安全审计（含剩余 `rsa` 公告）和部署配置；完整 WP00-R 与外部 TC 风险处置。完成每一项后，更新此矩阵的状态与证据链接，再评估生产发布。


## 当前停机行为与操作顺序

停机先阻止应用接收新事务，然后结束或交接当前应用持有的事务能力，再关闭 TM/RM 所有者。TM 的 shutdown 消耗管理器并等待自有网络任务结束。TM 关闭只关闭通讯，已创建的原全局事务仍由原 TC 状态、超时与持久 Launcher 权限决定后续动作。

RM 的 shutdown 先关闭客户端入口和连接，再取消并等待所有回调工作任务、网络监督器，最后在有界期限内等待本管理器日志的磁盘租约。资源动作取消后，已持久 Pending 保留。恢复使用原 XID、BranchId 和方向，先核对资源持久事实。

```mermaid
flowchart TD
    A[停止接收新业务] --> B[结束或持久交接在途事务能力]
    B --> C[关闭 TM/RM 通讯入口]
    C --> D[取消并等待 RM 回调与网络任务]
    D --> E{日志租约排空?}
    E -->|是| F[关闭返回成功]
    E -->|期限到达| G[返回日志未排空错误]
    G --> H[保留 Pending 与资源持久事实]
    H --> I[按原身份显式恢复]
```

当原 TC 已接收 BranchReport 且响应尚未返回时关闭 RM，调用结果保留为 UnknownOutcome，并携带原请求。保存该结果及 007 门闩，后续只按原身份核对。关闭后所有 ResourceClient 克隆的调用均在发送前失败。

日志应专用于当前管理器。外部仍持有租约时，关闭返回 RM journal work has not drained。释放外部租约后，日志可再次被原身份租用。网络关闭与本地 Future 取消之后，数据库的未决 COMMIT 继续按 004/006/007/013 的规则核对。

[停机 TCP 与日志租约证据](evidence/rm-shutdown-policy/README.md)覆盖当前组件行为。生产信号处理、应用业务流量排空、真实 Java TC/MySQL 停机故障及跨进程运维演练继续验收。

### 应用事务准入与排空契约

WP07 的应用接入使用共享 TransactionAdmission：配置正数容量，作用域开始前 try_enter 取得不可复制的 TransactionPermit，并持有到业务作用域完成或持久交接之后。容量耗尽与关闭分别返回独立错误。close 的共享关闭边界之后不再接纳新作用域；此前已接纳的许可仍有效。

drain 在开始等待前关闭准入，在有界期限内等所有许可释放。超时返回当时的在途数量，准入保持关闭，既不取消业务也不自动重放或结束事务。调用方只有在排空成功后，或已执行明确的持久交接策略后，才进入 TM/RM shutdown。许可 Drop 只证明本地作用域退出；未知事务结果仍须按原身份保留和恢复。

验收覆盖：容量与配置拒绝、所有克隆共享关闭、并行排空通知、超时不释放业务、取消排空不重开准入、任务取消和 panic 释放许可，以及关闭与并行准入边界。该组件不注册全局信号处理器；部署入口必须自行把停机信号接入此顺序。

### 可选应用停机信号契约

application_signals 功能由应用显式启用；默认不注册任何 OS 处理器。ApplicationSignals::install 在 Tokio 运行时内安装 Windows Ctrl-C/Ctrl-Break 或 Unix SIGINT/SIGTERM 接收器，recv 只返回信号种类，不自动取消业务或消耗 TM/RM 所有者。应用应在初始化时安装，在收到事件后关闭入口、排空作用域，再按原顺序 shutdown。监听 Future 取消后可以继续使用同一接收器。

Tokio 的底层信号处理器是进程级的，监听器 Drop 不保证恢复 OS 默认处理方式，连续事件可能合并。第二次信号不隐式强杀；强制终止策略须由应用明确实现。该入口不处理 Windows 服务控制事件或保证系统关机宽限期。

验收必须使用独立进程接收真实 OS 事件，并在原生 TM 的 Commit 响应被屏障持有时验证：收到事件后拒绝新准入、超时保留原许可及通讯所有者；释放响应后原事务完成、排空成功且 socket EOF。当前 Windows 现场与其他平台的验证状态分别记录，不以编译替代平台实测。
