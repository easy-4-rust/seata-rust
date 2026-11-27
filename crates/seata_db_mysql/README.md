# seata-db-mysql

WP05 真实 SQLx/MySQL 基础与受限本地 DML：有界事务、MDL/完整元数据、物理域、前后镜像、Undo 准备和持久化。无变化且无 TC 分支的 `commit_no_change` 已有受限入口；有变化业务的 `commit_changed` 仍在默认关闭的实验特性下，不是生产 COMMIT 能力。

[最新执行证据](../../docs/phase-2/evidence/mysql-execution-gate/README.md)与[契约](../../docs/phase-2/mysql-execution-images-contract.md)列明显式PK INSERT、UPDATE/DELETE、预算、取消/超时和失败回滚边界。INSERT内部临时键赋值需要CREATE TEMPORARY TABLES；不自动授权，不猜自增键。原[前镜像](../../docs/phase-2/evidence/mysql-row/README.md)及[查询间竞争](../../docs/phase-2/evidence/mysql-image-race-gate/README.md)保留历史执行范围。

固定 MySQL 8.4.13、`lower_case_table_names=2` 的选定现场用例覆盖 Java Undo 格式、同事务落库、原 TC 分支登记、提交尝试和本地结果恢复；[一阶段提交契约](../../docs/phase-2/mysql-first-phase-commit-contract.md)列出原回执与 004/006/007 持久边界。外部写入器锁覆盖、完整 TM 重建、全部 SQL/Undo 源案例、生产 TLS/最小权限及 WP05/06 完整门禁仍未完成。


[原生类型身份](../../docs/phase-2/evidence/mysql-type-gate/README.md)保留 18 种 `DATA_TYPE` 及 tiny 显示宽度，不转换原 `FieldValue`；该历史检查点的后续 codec/落库工作已在上述选定场景推进，不能据此声称完整 Java 源案例覆盖。

WP07 MySQL 连接配置增量：`MySqlOptions::new` 在连库前拒绝 `0.0.0.0`、`::` 等未指定目的 IP 和全空白用户名，保留合法主机名及字面 loopback 测试 TLS 选择。Rust 1.99/1.85 的 5 项配置契约测试、45 项库测试与全目标严格 Clippy 均通过。此校验不代替真实 TLS/认证握手和生产部署发现；有变化提交仍由默认关闭的 `experimental_changed_commit` 提供，WP05/06/07 Gate 未闭合。


## 恢复控制事务只读指标

MySqlDataSource::recovery_metrics_sampler 与 MySqlDataSourceManager::recovery_metrics_sampler 签发 MySqlRecoveryMetricsSampler。同一数据源及其全部克隆共享六种 MySqlRecoveryOperation 的固定计数；重建管理器不清零，新建数据源归零。采样器不持有连接池、资源或恢复权限，数据源关闭后仍可读取。

MySqlControlMetrics 的 active 是在途调用；completed、unknown_commit、before_commit_error、cancelled 互斥结算一次内部 control 调用。completed 不证明业务提交，可能只是持久登记、方向绑定或只读事务。成功结果的 applied/unresolved 单独计数，Applied 包含读取已有终态，不能作为效果执行次数。取消不推断提交边界，提交前错误不证明旧业务没有效果。快照逐字段读取，累计饱和不回绕。

seata-observability 显式启用 mysql 特性后，with_mysql_recovery 可独立导出固定 42 条序列。默认导出器不引入 MySQL/SQLx 运行依赖。此增量覆盖恢复管理器控制事务；一阶段业务提交、独立 Undo API 和完整生产观测门禁仍需分别验收。见 [真实数据库与原 TC 证据](../../docs/phase-2/evidence/mysql-control-metrics/README.md)。


## 可选原生 XA

`xa` 特性显式启用独立 `seata-xa` 身份模型和 `MySqlXaDataSource`。先按既有 TLS/实例/schema 验证建立原 MySqlDataSource，再构造 XA 入口。start 消费原 XaBranchXid，execute 沿用参数化 DML/预算及当前 InnoDB/约束检查；prepare 消费活动作用域，发送同连接 END/PREPARE。准备后行锁由 MySQL 持有，原连接退休，新连接可显式 commit_prepared 或 rollback_prepared。XA 不写 AT Undo；禁止 ONE PHASE 和活动作用域的普通本地 COMMIT。

原生操作错误分为本次操作 NotSent、命令可能发送后的 Unknown、只读观察失败与中毒。prepared_observed 只判断当前真正 XA RECOVER 中是否存在完整三分量原身份；缺行或 XAER_NOTA 不能确认原提交/回滚，也不授权自动重试。Drop/取消关闭活动连接，不把 Prepared 当作已回滚。调用方保留原身份并遵循 TC 的事务决定。

受管理入口 MySqlXaResourceManager 已接入正式 XA BranchResource 和单次 TC 登记，014 显式新表保存原回执身份、防重启行、原方向与已确认终态；仅实际 Prepared 可以执行同方向结束，终态重读不再次发送原生控制。MySqlXaEnlistedTransaction 只允许登记后 DML 与消费式 Prepare，不自动发送 AT 的 PhaseOne_Done；错误保留原真实回执。缺登记/Prepared 和原生响应丢失仍保持未知，不能用任意字节身份冒充登记。结束已生效但终态记录未确认的恢复、旧活动会话围栏、完整 XA RM/Java TC 故障门禁与生产部署仍开放。见 [实施契约](../../docs/phase-2/mysql-xa-contract.md)。AT 默认生产 COMMIT 门槛不因此改变。


受管理 Prepare 还要求显式部署 015 提交见证表；原 ACTIVE XA 在 END/PREPARE 前写入精确原身份见证，与业务一起提交或回滚。原生 Commit OK 丢失后，精确匹配的已提交见证可恢复同一原方向终态，不重发 COMMIT。缺见证仍 Unknown；旧 014 Prepared 没有见证时不会被补造成功。回滚响应丢失、旧活动会话围栏与完整故障门禁继续开放。


受管理 Prepare 还要求显式部署 016 生产者/Prepared 结束意图表：原见证写入成功后先持久生产者证据，重新获取方向门闩后才 END/PREPARE。Rollback 对精确 Prepared 的观察与回滚意图须先持久确认；之后恢复同时要求原意图、Prepared 缺失及提交见证缺失，才完成原回滚。只有生产者证据、缺失/损坏的结束意图或可见提交见证均不能确认回滚。未 Prepared 的活动会话和外部写入权限等完整生产门禁继续开放。


017/018 显式登记新作用域的 guard 协议。任何业务之前，原 XA 必须在原物理连接取得事务内 guard；可取消的独占会话槽通过持久决定监视器退休本库持有的旧连接，不使用远程 KILL。Rollback 恢复在原 guard 唯一键取得持久屏障后，重查 Prepared/提交见证并与原终态一起提交。旧协议记录、未释放的 guard、身份/结构冲突均不能补成成功；被暂停/分区的会话保持未知直到实际可取得屏障。完整强制会话清理、性能/运行手册和生产权限仍开放。


正式 XA 回调观察器现复用受控 BranchCallbackReceipt 处理早到 Rollback：先竞争原登记唯一键，排除孤立记录/Prepared 后，原子持久否定启动终态与 guard。迟到的真实登记回执和克隆不能 START；若原登记先获胜则交回正常恢复，不从重复键猜结果。手工 apply/recover/dispatch 缺记录仍 Unknown，不能签发该屏障；回调能力也不转换为 RegisteredBranch。已有孤立数据保持 Pending，修复后才重新证明。

### 显式 TCC 动态上下文报告

启用 `tcc` feature 后，部署 `020_seata_rs_tcc_context_v1.sql` 并调用 `MySqlTccResourceManager::with_context_reporting()`，才会使用独立报告清单；默认静态路径不要求此表。控制池至少需要两个连接，并保持现有操作期限。清单最多保存原登记版本 0 和 64 个不可覆盖报告版本，包含原身份、实例、schema_version 和完整原字节摘要，不代表业务已经提交。

业务可覆盖 `TccBusinessAction::try_action_with_context` 修改上下文；立即报告显式等待 `TccTransaction::report_context`，其余变更在业务返回后的 finally 路径报告。原 updated/延迟标志不证明网络送达，完整原值字节变化仍需要报告。报告确认后才允许发送业务/Fence COMMIT；报告错误、取消或被捕获的报告错误会封闭原事务。业务返回 false/错误仍回滚本地业务，但此前成功的上下文报告可独立存在。旧只读 Try 实现保持兼容。

`register_and_try` 返回未改写的原成功登记回执；预先取得回执的调用方使用 `try_registered_with_context(client, receipt)`，并在取消 Future 之前自行保留原回执。普通 `try_registered` 没有正式报告客户端，不能用于已开启报告的资源。`MySqlTccTryError::ContextReport` 保留原回执、原报告错误及已开始请求的边界；Deadline 分别保留登记、报告和本次业务 COMMIT 的可能发送状态。不自动重发登记、报告或业务。

原连接回调先核验清单和当前 Fence，之后才能以原登记身份复用文件 Pending/Completed；实际业务参数仍来自验证后的原回调，不全局忽略 application_data 冲突。摘要损坏保持可重试；恢复原字节后同一原分支可继续。原 Undo、019 Fence 和 AT/XA 格式均不迁移。完整崩溃/并发/拓扑、util 自动报告便利入口和生产权限/保留/负载门禁继续开放。

新动态 Fence 使用既有 schema_version 字段的协议值 2；当前静态路径只接受原协议 1，误用静态配置不能执行动态分支的旧参数。019 表结构、旧记录和默认值不改写，020 清单自身版本仍为 1。恢复时必须保留所选协议配置。

当前运行时的 TCC 回调复用 Completed 前还会验证同方向终态 Fence，控制状态缺失、回退或摘要异常时不会仅凭日志返回成功，也不会重放业务。此校验不承诺旧二进制任意降级的行为；恢复须维持相应报告配置与版本。
# TCC util 自动报告

可变 Try 可调用 `seata_tcc::BusinessActionContextUtil::add_context_map(context, transaction, Some(&values)).await`；默认延迟设置在 Try 返回后发送，业务显式设置 `delayReport=Some(false)` 后，参数变化自动通过同一受控事务报告一次。它复用既有原回执、持久完整版本和类型化错误边界；成功报告不表示 Try 已提交。预算拒绝不留下半批参数，捕获网络错误也不能让已封闭的 Try 继续提交。原参数本地修改方法仍只修改本地值；需要自动报告时使用 util 入口。
