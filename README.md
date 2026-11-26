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

正式 Rust workspace 已启动：[seata-protocol](crates/seata_protocol/README.md)现有显式线缆枚举、类型化错误、有界字段读写、27类载荷及完整V1帧/有序可空HeadMap/心跳编解码。传输、TM、RM 已通过各自批准的初期配置验收；MySQL AT 与生产门禁仍在实施，当前不是完整V1客户端。

[seata-transport](crates/seata_transport/README.md)已通过[WP02传输组件验收](docs/phase-1/evidence/transport-recovery/README.md)：87正式测试＋3联调辅助测试在Rust1.99/最低1.85分别通过，固定JavaTC六登记及三丢响应场景通过。已实现有界TCP池/重连、原代际回包、未知结果/绝对期限、心跳及资源就绪。

[seata-tm](crates/seata_tm/README.md)已通过[WP03完整组件验收](docs/phase-1/evidence/tm-gate/README.md)：28TM＋87传输回归及另2辅助测试双版本通过，原JavaTC六基础场景＋三真实TM响应丢失通过。正式RM后续验证见下，SQL/Undo及完整V1继续实施。

[seata-rm](crates/seata_rm/README.md)已通过[WP04首期验收](docs/phase-1/evidence/rm-gate/README.md)：正式资源目录、注册生命周期、单次分支调用、原连接回调、持久日志及强杀恢复；45RM纳入234项双版本组件回归。新执行原JavaTC正常/重复/断连各提交和回滚六例，资源均只生效一次；11实现与10观察器削弱检查通过。初始化缓存回调、完整Ready后转发的规则不变。下一步MySQL AT phase1与SQL/Undo，完整V1及其余计划继续。

[seata-at](crates/seata_at/README.md)已落实[首批SQL识别验证](docs/phase-2/evidence/at-sql/README.md)：完整单句与支持/拒绝矩阵、AST吞词核对、参数顺序和前置资源预算；18项新增测试纳入252项双版本回归，10隔离削弱断言通过。[真实MySQL基础](docs/phase-2/evidence/mysql-foundation/README.md)已验证元数据、复合PK、MDL及失败期限：双版本各259工作区测试与8项实际数据库测试。[精确行值模型](docs/phase-2/evidence/row-image/README.md)已通过双版本各273项回归与10项削弱断言；[MySQL物理域与UPDATE/DELETE前镜像](docs/phase-2/evidence/mysql-row/README.md)已通过各282项工作区与19项真实数据库测试；[两查询间并发变化](docs/phase-2/evidence/mysql-image-race-gate/README.md)已取得双版本新现场与两目标负控；INSERT镜像、业务SQL、after image及Undo原子事务继续实施，WP05十四项和完整V1尚未完成。

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
