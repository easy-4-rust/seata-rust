# 全计划当前状态核对（2026-11-28）

范围保持为[原架构计划 Phase 0–7](SEATA_RUST_ARCHITECTURE_PLAN.md)和[V1 WP00–07](../phase-0/v1-implementation-acceptance-checklist.md)。本文件是当前工作树的导航与差距记录，不替换正式需求、不豁免门禁。历史 README 中“尚未接入”的描述必须按对应日期理解，不能据此重复开发已经存在的能力。

当前 Cargo 工作区包含 protocol、transport、tm、rm、at、datasource、db_mysql、sqlparser、test、observability、xa、tcc 十二个正式 crate。未发现 Rust TC 服务端、PostgreSQL、Saga、集群/发现服务或 CLI 的正式工作区成员。不能把参考 Java 服务端、Python 运行器或测试夹具计为这些正式交付。

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
