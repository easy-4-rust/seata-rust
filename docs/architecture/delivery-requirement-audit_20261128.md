# 全计划当前状态核对（2026-11-28）

范围保持为[原架构计划 Phase 0–7](SEATA_RUST_ARCHITECTURE_PLAN.md)和[V1 WP00–07](../phase-0/v1-implementation-acceptance-checklist.md)。本文件是当前工作树的导航与差距记录，不替换正式需求、不豁免门禁。历史 README 中“尚未接入”的描述必须按对应日期理解，不能据此重复开发已经存在的能力。

当前 Cargo 工作区包含 protocol、transport、tm、rm、at、datasource、db_mysql、sqlparser、test、observability、xa、tcc 以及 orm_seaorm 十三个正式 crate。未发现 Rust TC 服务端、PostgreSQL、Saga、集群/发现服务或 CLI 的正式工作区成员。不能把参考 Java 服务端、Python 运行器或测试夹具计为这些正式交付。

| 原要求 | 当前源码与权威证据入口 | 未完成或证据不足 |
|---|---|---|
| Phase 0：参考审计与兼容基线 | Phase 0 ledger、状态矩阵、ADR；WP00-P 与 WP01 初期 profile Gate | 完整 WP00-R、原 Go 默认路径差异、外部 Java FILE/RAFT 等问题的完整闭环；候选补丁不等于上游发布已修复。 |
| Phase 1：协议、传输、TM、上下文 | `seata_protocol`、`seata_transport`、`seata_tm`；WP01–03 组件 Gate | 完整部署及跨版本/语言运行验收继续依赖后续门禁。 |
| Phase 2：RM、MySQL AT、Undo、锁 | `seata_rm`、`seata_at`、`seata_db_mysql`；WP04、WP05 各现场 ledger、WP06 十项及 Undo SHA-256 原事务恢复证据 | 默认有变化 COMMIT 仍关闭；完整 SQL/外部写入器/物理资源域准入及 WP05 总 Gate 尚未关闭。早期“只有识别或内存 Undo”已经不反映当前实现。 |
| Phase 3：恢复、重试、幂等、并发、负载、跨语言 | 已有 AT 恢复/丢响应、TLS、日志/指标、停机、原 Java TC 证据；本轮新增[并发负载契约](../phase-2/at-concurrent-load-contract.md) | 本轮是有限并发现场，不能替代长时负载、生产容量、生产权限/证书/告警/值守及完整跨语言测试。 |
| Phase 4：XA、两阶段、TCC、Fence、空回滚、悬挂 | `mysql_xa_resource_manager.rs`、`mysql_xa_enlisted_transaction.rs`、`mysql_tcc_resource_manager.rs`；[XA 契约](../phase-2/mysql-xa-contract.md)、[TCC 契约](../phase-2/tcc-contract.md)及 `docs/phase-4` 最新 ledger | 正式 XA/TCC 已实现，不应按照早期 native-only ledger 重新创建；完整模式门禁仍需覆盖对应剩余兼容、拓扑、权限、并发和故障要求。 |
| Phase 5：独立 Rust TC | 原计划第七节规定 Session Store、Lock Manager、恢复、重试、超时和三语言客户端 | 当前缺正式实现与验证；遵守原计划“V1 稳定运行后”启动条件，不把 Java TC 复用计作 Rust TC。 |
| Phase 6：集群和基础设施 | 原计划第八节及 Phase 6 | 集群、注册发现、配置中心、多存储、管理 API、完整监控审计均未取得整体实现与验收证据。现有本地指标服务不是 TC 管理服务。 |
| Phase 7：完整生态 | 原模块差异矩阵和 Phase 7 | Saga、PostgreSQL、更多 ORM/RPC、CLI、插件和 Java 2.x 完整功能对照仍未交付。 |

后续执行先推进阻挡 V1 稳定性的真实缺口：对照 WP05 完整 SQL 与所有写入器准入要求补证/修复，再完成 WP07 的部署、安全、长期负载与运维验收。XA/TCC 只为已命名的未闭合要求补工作，不通过无限细分相同故障切点替代上述交付。达到既有 V1 稳定性条件后，按原路线实现 Phase 5–7。每次完成声明须绑定实际源码、运行输入和原始结果；本文件不构成任何总阶段已经完成的声明。

时间类型后续核对：[DATE/TIME/TIMESTAMP 原实现保真契约](../phase-2/mysql-temporal-compatibility-contract.md)记录原读取缺口。TIMESTAMP 已有独立原生身份、显式 UTC 回调与跨进程恢复；DATE/TIME 已新增显式无损装饰器、正式值模型及原 TC 下双 Rust 回调和生产者死亡后恢复。不能继续按早期观察重复开发这些能力。[完整配置准入](../phase-2/mysql-jdbc-profile-admission-contract.md)现有 Rust 原 TC 现场及 Java 原连接数据库探针；后者数据源身份使用替身，完整 Java TC/RM 与严格入口集成、Java RM 时间补偿、所有写入器准入仍待验证，不关闭完整 WP05。

后续已取得[原 Java TC/RM 的严格入口与 DATE/TIME 补偿证据](../phase-2/evidence/java-writer-jdbc-profile-tc/README.md)：六码共十二笔全局事务和六十次错误绑定拒绝，选定精确 DATE/TIME 普通列及主键的原 RM 回调通过。新入口的原 Java 集成不再是未实施项；下一步核对两种语言在同一精确锁键上的混合竞争与准入，继续保留完整 SQL、生产权限/认证、WP07 和 Phase 5–7 的缺口。

精确 DATE/TIME 的[双向混合竞争](../phase-2/mysql-exact-temporal-mixed-lock-contract.md)已取得双 Rust、配置 5/6 的真实原 TC 证据。反向还核验正式已提交阶段一的 checksum 和历史见证、Java 原 RM 的实际补偿及围栏保留。下一步继续对照 WP05 的完整 SQL/写入器要求和 WP07 生产门禁核对实际剩余项；不把选定键的互操作验证等同于全部写入器准入完成。

[首期 SQL 矩阵](../phase-2/evidence/initial-sql-matrix/README.md)现已取得双版本、提交/补偿的 88 笔实际全局事务与原 Java parser 重读证据，关闭对应分类器/明确矩阵项。完整 Java SQL 迁移仍开放。该长时现场还记录原 TC 的 111 单向清理指令；`CallbackHandler` 只接受同步分支回调，下一项具体工作是接入并验收正式 RM 的 Undo 定期清理，继续保留生产权限/认证等门禁。

后续[Undo 定期清理增量](../phase-2/evidence/undo-retention/README.md)已接入正式 RM/MySQL，并通过未修改原 TC 的 180 秒定时任务。双 Rust 各删除 6107 条过期原行，UTC 边界与原 Java 三批删除一致；错误绑定和触发器拒绝、恢复清理及同一连接后续分支回调均有实际证据。双版本全工作区各 831 测试通过、413 忽略。真实权限撤销、批次提交未知结果和生产运维仍开放；不因此关闭 WP05/WP07，更不把本增量算作 Phase 5–7 的实现。

[单表 SQL 别名](../phase-2/mysql-sql-alias-contract.md)现已补齐原 Java 支持的显式/省略 AS、合法限定列及带别名前镜像。扩展 SQL 矩阵在双版本提交/补偿下通过 112 笔真实全局事务，全部 Undo 由原 Java parser 读取，六个新增输入还直接对照原 Druid 识别器。未知前缀、跨 schema、重复赋值和连接仍拒绝。该项不再列为未实现，但 JOIN/子查询等完整 SQL、全部写入器、生产准入及后续阶段仍需继续交付。

原计划要求的独立 SeaORM 层已新增[正式实体 DML 适配](../phase-2/evidence/seaorm-mysql/README.md)，保持原 MySQL 本地事务、Undo、摘要、锁和提交/恢复门槛。双版本十二笔真实 Entity INSERT/UPDATE/DELETE 通过，DECIMAL/二进制/中文 JSON/NULL 全字段及原 Java parser 核验通过；全工作区各 840 通过、414 忽略。当前仍未提供原生 ConnectionTrait、完整读写/模型返回、生成 ID、关联和 2.x，不能标记完整 ORM/生态完成。最新全 lock 审计报告 rsa、time、libsqlite3-sys 三项；当前正常构建未启用后两者，但没有移除或忽略告警，生产安全门禁仍开放。下一步应依据最新上游修复和实际依赖链处理真实安全缺口，继续完整 ORM、SQL 和后续阶段交付。

后续 [RSA 认证处置](../phase-2/evidence/mysql-rsa-auth/README.md)以明确的本地 SQLx 0.8.6 维护补丁替换旧 RSA 依赖，实际图与 lock 不再包含 rsa，Rust 1.85 保留。两版、三种公钥、两种认证插件/TLS 的十八笔正式 AT 事务及八项拒绝通过；真实认证帧、持有连接时 TLS 会话、原子 Undo/SHA 和原 TC 回调均独立核验，工作区各 840 passed / 415 ignored、严格 Clippy 通过。time/SQLite 告警仍保留，未发布消费者不会自动继承根 Cargo patch；发布集成、完整 ORM/SQL、生产运维及后续阶段继续开放。

[原业务语句回执](../phase-2/evidence/mysql-statement-result/README.md)已从 StatementCallback 贯通正式事务和 SeaORM `execute_with_result`，u64 原始影响行数/ID 不经后续会话查询或镜像推算，旧行数接口兼容。底层大整数回执、会话值变更、批量首 ID、错误/回滚，以及双 Rust 十二笔实体原 TC 提交/补偿均实际验证；各 840 passed / 416 ignored、严格 Clippy 通过。本项只是生成主键能力的前置交付，正式自增表/缺省键镜像定位仍拒绝并待实施，原生 ConnectionTrait、完整读写和 Phase 5–7 不变为已完成。

[正式单行自增主键](../phase-2/evidence/mysql-auto-increment/README.md)现已从元数据准入推进到同一原 INSERT 回执定位镜像，保留原 Java Undo 和旧构造兼容。双 Rust、步长 7/偏移 3、十三类输入的 52 笔原 TC 提交/补偿、三个执行前拒绝、属性新增/移除快照拒绝与原 Java parser 均通过；零键补偿恢复及大于 i64 上限的键有实际证据。全工作区各 845 passed / 417 ignored、三个受影响包严格 Clippy 通过。正式单行自增镜像不再列为未实施；生成键批量、全部整数/JDBC 配置认证、原生 ORM 连接/模型/查询等完整范围继续交付，不关闭 WP05/WP07 或后续阶段。

[原生借用式 SeaORM 连接](../phase-2/evidence/seaorm-borrowed/README.md)已实现可选 ConnectionTrait、同原事务有界 RR 查询及真实 Entity 模型插入/更新/删除；释放借用后继续原提交/补偿，不提供独立事务控制。双 Rust 十二笔原 TC 事务、原 Java Undo parser、原生列索引/时间读取、RR、预算拒绝及 MDL 取消均通过；各 850 passed / 418 ignored，严格 Clippy 通过。默认适配仍无 ORM 驱动，核心无 ORM 依赖。原生 ConnectionTrait/基本模型返回不再是完全缺失项，关系/聚合/更多表达式/流式/嵌套事务/迁移/2.x、完整 SQL 和生产范围仍开放。
