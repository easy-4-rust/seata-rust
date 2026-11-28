# rbatis 发布版与双 Rust 兼容基线

权威现场：[20261128_105604_0aaebe00](20261128_105604_0aaebe00/report.json)。该现场不是 MySQL/TC 测试，正式 adapter 和数据库验收仍为 pending。

- rbatis/rbdc 4.9.11、宏驱动/codegen 4.9.8、rbs 4.8.4；四份维护副本与发布包来源摘要核对通过，宏驱动补充声明所需 Apache 2.0 标准许可证。
- Rust 1.85.0/1.99.0 各八项原生 CRUD/三种 SQL 宏/精确值/类型转换/分页测试通过，严格 Clippy 通过。
- 使用完全相同的测试源码、恢复原发布版宏驱动后，三个调用因直接进入无池元数据引擎而失败，其余五项通过；不是用源码搜索替代实际运行。
- 197 个运行输入的起止摘要一致，41 个原始证据成员的 catalog SHA-256 为 `5a57d7a1c219df76e3e054e6765757ce04e8bcea528c049430aff68b64747ef3`。
- RustSec 快照 `c5279d0b0c9e8b70df666eb6d1edbabcfd1176bd` 实际报告 `RUSTSEC-2026-0009`：time 0.3.45 经 fastdate 成为该独立探针的正常依赖。审计返回 1，未忽略。正式根 Cargo.toml/Cargo.lock 未改变，根工作区尚未引入 rbatis。

复验：`python -B tools/formal-source-junit/validate_rbatis_compatibility.py docs/phase-2/evidence/rbatis-compatibility/20261128_105604_0aaebe00`。重跑使用 `rbatis_compatibility.py`，详见 [独立探针](../../../../tools/rbatis-compatibility/README.md) 和 [接入契约](../../rbatis-mysql-contract.md)。

本项只完成接入前的依赖/宏可行性基线，不关闭正式原事务执行器、输入/输出预算、真实 MySQL/原 Java TC 提交补偿、Undo/SHA/锁/恢复或生产安全门禁，更不关闭 WP05/WP07 与 Phase 5–7。
