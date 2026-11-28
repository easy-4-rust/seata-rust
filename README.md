# Seata-Rust

![Rust](https://img.shields.io/badge/language-Rust-orange?logo=rust)
![License](https://img.shields.io/badge/license-Apache--2.0-blue)
![Roadmap](https://img.shields.io/badge/roadmap-V1%20Client%20%E2%86%92%20Rust%20TC-success)

**Rust-native distributed transaction infrastructure in the Apache Seata ecosystem.**

Seata-Rust follows a staged strategy:

1. **V1 — Java TC compatibility:** implement Rust TM/RM and protocol compatibility, reusing the mature Java Seata Transaction Coordinator.
2. **V2 — Native Rust TC:** progressively implement transaction coordination, persistence, recovery, locking and cluster capabilities in Rust.
3. **Long term — Seata ecosystem coverage:** expand transaction modes, database/ORM adapters, discovery, observability and multi-language interoperability.

## Architecture & Roadmap

正式 Rust workspace 现包含 Protocol、Transport、TM、RM、AT、MySQL、XA、TCC 和可观测性等十三个 crate。WP01–WP04 已通过批准的初期组件与固定 Java TC 验收；完整 V1 和生产发布门禁仍开放。

MySQL AT 已实现真实 SQL 执行、元数据与前后镜像、Java Undo 格式、原子持久化、SHA-256 校验、正式回调及跨进程恢复。[显式 UTC 的 DATETIME 精确读写及主键](docs/phase-2/evidence/mysql-datetime/README.md)已取得原 Java/双 Rust 验证；[TIMESTAMP 原生时间点及正式回调](docs/phase-2/evidence/mysql-timestamp/README.md)也已通过选定 UTC 现场，[强杀后独立进程恢复](docs/phase-2/evidence/mysql-timestamp-restart/README.md)已补验；非主键 JSON 列已通过[原 Java 格式与正式补偿对照](docs/phase-2/evidence/mysql-json/README.md)。有变化 COMMIT 仍需明确启用 `experimental_changed_commit`，完整 WP05 与生产准入尚未完成。详见[能力与恢复指南](docs/phase-2/v1-feature-matrix-and-recovery-guide.md)。

可选 `xa` / `tcc` 已提供正式 MySQL 资源：XA 登记、PREPARE/二阶段及恢复；TCC Fence、空回滚、悬挂防护与动态上下文报告。固定原 Java TC、Rust 1.99 / 1.85 和真实 MySQL 已有选定故障矩阵证据，完整模式验收仍开放，见 [XA 契约](docs/phase-2/mysql-xa-contract.md)和 [TCC 契约](docs/phase-2/tcc-contract.md)。

当前范围与原 Phase 0–7 差距见[全计划状态核对](docs/architecture/delivery-requirement-audit_20261128.md)。Rust TC、集群和完整生态尚未交付。

正式 `seata-orm-seaorm` 已接入 SeaORM 1.1 实体 DML 构建器，在原 MySQL AT 事务中保持有序精确绑定。双 Rust 的十二笔真实提交/补偿、原 Java Undo parser 读取及完整字段核验通过；可选借用式原生 ConnectionTrait、基本模型返回与有界快照读取现已通过双版本原 TC 验证，完整 ORM 其余范围继续交付。[原生连接验收](docs/phase-2/evidence/seaorm-borrowed/README.md)。[接入说明](crates/seata_orm_seaorm/README.md)、[验收证据](docs/phase-2/evidence/seaorm-mysql/README.md)。 现新增原语句计数/ID 回执 API，双版本大整数与十二笔事务通过；正式单行自增键镜像定位及零键补偿现已通过双版本 52 笔原 TC 事务，完整 ORM 继续实施。[自增验收](docs/phase-2/evidence/mysql-auto-increment/README.md)。[回执验收](docs/phase-2/evidence/mysql-statement-result/README.md)。

当前根构建通过明确的本地 SQLx 认证补丁移除旧 `rsa`，保留 Rust 1.85；十八笔真实认证/AT 事务与八项拒绝通过。该 root patch 尚需发布消费者集成，time/SQLite 锁项告警及完整生产门禁仍开放。[处置范围和证据](docs/phase-2/evidence/mysql-rsa-auth/README.md)。

- [全计划交付进度](docs/delivery-progress.md) — Phase 0–7 完整范围、当前证据和未完成门槛。
- [V1 能力矩阵与恢复指南](docs/phase-2/v1-feature-matrix-and-recovery-guide.md) — 当前可核验配置、实验性 MySQL 恢复边界及操作约束。

- [Seata-Rust Architecture & Roadmap](docs/architecture/SEATA_RUST_ARCHITECTURE_PLAN.md) — overall vision, module matrix, execution flows, staged roadmap and V1 acceptance goals.
- [Phase 0 Research Index](docs/phase-0/README.md) — pinned upstream snapshots and all Phase 0 deliverables.
- [Java TC State Transition Matrix](docs/phase-0/java-tc-state-transition-matrix.md) — global/branch states, retry, timeout and recovery.
- [Java TC Retry-Timeout Recovery Audit](docs/phase-0/java-tc-retry-timeout-recovery.md) — storage-mode asymmetry and operator retry semantics.
- [V1 Payload Golden Vectors](docs/phase-0/protocol-golden-vectors.md) — codec-body and serialized-payload vectors with explicit certification status.
- [Java V1 Frame Compatibility](docs/phase-0/protocol-v1-frame-compatibility.md) — Java V1 frame source audit and runtime test gates.
- [Go Golden Vector Runner](tools/go-golden-vectors/verify.sh) — reproducible pinned Go codec and frame verification harness.
- [Java V1 Frame Runner](tools/java-v1-golden/verify.sh) — reproducible pinned Java decoder/encoder frame round-trip test.
- [TM/TC/RM Call Chains](docs/phase-0/tm-tc-rm-call-chains.md) — method-level flows and failure matrix.
- [Java TC Request Exception Matrix](docs/phase-0/java-tc-request-exception-matrix.md) — per-request exception and response semantics.
- [Seata-Go Transport Semantics](docs/phase-0/seata-go-transport-timeout-semantics.md) — timeout, correlation, late responses and RM reconnect.
- [V1 Implementation Acceptance Checklist](docs/phase-0/v1-implementation-acceptance-checklist.md) — dependency-ordered V1 work packages and test gates.
- [Phase 0 Validation Ledger](docs/phase-0/validation-ledger.md) — pinned source SHAs, runtime evidence and WP-00 gates.
- [Protocol Compatibility CI](.github/workflows/phase-0-protocol-compatibility.yml) — automated pinned Go and Java V1 protocol tests.
- [Java TC Live Integration Plan](docs/phase-0/java-tc-live-integration-plan.md) — Phase 0.9 live TC test topology and acceptance gates.
- [V1 TM/RM Registration Wire Contract](docs/phase-0/registration-v1-wire-contract.md) — pinned Java/Go RegisterTM/RegisterRM wire layout and V1 server channel selection.
- [Java TC Integration Harness](tools/java-tc-integration/README.md) — pinned Java TC FILE build/start/stop/reset scripts.
- [Java TC Bootstrap CI](.github/workflows/phase-0-java-tc-file-smoke.yml) — builds and starts the pinned Java TC in CI.
- [RSeata Reference Disposition](docs/phase-0/rseata-module-disposition.md) — third-party Rust reference audit, not a migration plan.
- [Architecture Decision Records](docs/adr/README.md) — accepted and proposed architecture decisions.

### Phase 0 scope

Phase 0 is research and specification only. It establishes source provenance, protocol facts, state semantics, compatibility risks and ADRs before implementation begins. **No application/business code is being started in this phase.**

The Phase 0 documents distinguish source-verified facts, test-verified behavior, inference and unknowns. A README claim, source directory, Cargo feature or matching enum name is not considered proof of compatibility.

## Reference Projects

- [Apache Seata Java 2.x](https://github.com/apache/incubator-seata/tree/2.x)
- [Apache Seata-Go v2](https://github.com/apache/incubator-seata-go)
- [RSeata](https://github.com/oulover/rseata)

## Project Status

WP01–WP04 已在批准的初期配置范围内取得组件和固定 Java TC 验收证据；WP05–WP07、完整 WP00-R 和生产发布门禁仍开放。当前不声明完整 V1 或生产可用，详见[能力矩阵](docs/phase-2/v1-feature-matrix-and-recovery-guide.md)。

## License

Apache-2.0. Review upstream NOTICE, license headers and third-party dependency obligations before reusing upstream code or definitions.


WP05新增[受限真实业务执行、前后镜像与内存Undo准备](docs/phase-2/evidence/mysql-execution-gate/README.md)；同事务Undo持久化、正式注册/提交未知及全部后续计划继续。


WP05补齐[原生MySQL类型身份与选定原Java Undo格式对照](docs/phase-2/evidence/mysql-type-gate/README.md)；双向Rust codec及同事务Undo继续。

WP05推进[SQL parser core原对象迁移](docs/phase-2/evidence/sqlparser-index-gate/README.md)：基础值对象、ColumnMeta、IndexMeta及异常传播已验证，累计57个原案例双工具链差分匹配；完整31对象／76案例迁移仍在实施。 [TableMeta主键集合依赖](docs/phase-2/evidence/sqlparser-table-prerequisites-gate/README.md)已验证，主对象仍待完成。 [Locale上下文依赖](docs/phase-2/evidence/sqlparser-locale-gate/README.md)已差分验证，默认Locale及表映射集成继续实施。


[TableMeta首期差分](docs/phase-2/evidence/sqlparser-table-gate/README.md)：原76案例双工具链MATCH，TableMeta仍PARTIAL，源对象13完整/1部分/17缺失；完整交付继续OPEN。

可选原生 ORM 连接现支持受控单表 COUNT(*)、原生分页/分页流和只读 LIKE；双版本实际统计 3006 行并验证十二笔原 TC 实体提交/补偿，全工作区各 853 passed / 419 ignored。[分页验收与范围](docs/phase-2/evidence/seaorm-pagination/README.md)。完整 ORM 与其余计划继续实施，rbatis 尚未接入且未被排除。
