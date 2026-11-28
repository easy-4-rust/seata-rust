# Rust 已提交持锁，原 Java 精确时间写入器竞争

权威包：[20261128_133635_d5470cdb](20261128_133635_d5470cdb/result.json)。catalog SHA-256：`87c385aff3d1d4fddfc393bced4c07691764f6d5f53140dd27c76ecf2f2baea6`。[对应契约](../../mysql-exact-temporal-mixed-lock-contract.md#rust-已提交持锁原-java-竞争)。

固定原 TC JAR、原 Java TM/RM/DataSourceProxy/UndoLogManager、Connector/J 8.0.27、Jackson 2.18.3 和 MySQL 8.4.13。两个 Rust 工具链为 1.99.0、1.85.0。新增目标调用正式 AT 准备、提交前门闩及实验性有变化阶段一提交/报告 API；本轮没有修改 Rust 生产组件或原 Java 实现。

| 维度 | 实际覆盖 |
|---|---|
| Rust 工具链 | 1.99.0 / 1.85.0 |
| 完整配置 | 5 / 6 |
| Rust 决策 | commit / rollback |
| 竞争主键 | DATE `1582-10-05` / TIME(6) `-25:01:02.123456` |

八组共产生 40 个不同的全局事务：8 个 Rust 持锁事务、16 个被拒绝的 Java 事务、16 个释放后成功登记的 Java 事务。原 TC 的 16 次拒绝均来自实际 BranchRegister；Java 本地变化撤销，Rust 阶段一业务、分支、两个锁键及 Undo 原字节保持不变。

Rust 阶段一独立 SQL 同时核对配置、`first_phase_committed=1` 和 `undo_sha256=SHA256(rollback_info)`。原 BranchReport 为 PhaseOne_Done。Rust 最终提交保留业务值，补偿恢复原值；两种结果均经过实际原 TC 回调，数据库恢复记录保持正确终态和阶段一历史见证。

释放后，Java 对 DATE/TIME 分别完成真实阶段一，原 Undo 保留 JDBC 91/92 与自然 String。独立 SQL 逐字核对原 XID、branch ID、资源、锁主键和可见业务。随后原 Java RM 实际补偿，各次业务都恢复到 Rust 的最终结果，TC global/branch/lock 与正常 Undo 均为零。

原 TC 的 DB 会话使用既有联试的 `retry-dead-threshold: 1000`。固定原源码表明默认阈值为 70 秒，正常 DB 会话结束可延后处理；源码随包保存。此参数只定义自有现场的清理时间，不作为生产延迟结论。

重复/空 Undo 补偿可以保留原 GlobalFinished 围栏。完整现场四个 Rust 回滚组各保留一个对应已完成 owner 的围栏；每个原回调都按真实请求 ID 配对核验。原始围栏状态、字节和上下文单独保存。后续独立测试只在客户端退出后审计并重置已结束的自有夹具围栏；最后一组实际围栏保留在停止的自有 MySQL 数据目录。正常 Undo 清空与围栏保留分别验收。

```powershell
python -B tools/formal-source-junit/java_exact_temporal_rust_owner.py
python -B tools/formal-source-junit/validate_java_exact_temporal_rust_owner.py docs/phase-2/evidence/exact-temporal-rust-owner-java/20261128_133635_d5470cdb --full
```

包内保存全工作区 Rust 源码输入、Java 源码、编译类、双版本构建/严格 Clippy 原始结果、二进制及依赖绑定、实际类来源、原始帧和独立数据库结果。独立校验器检查八组完整覆盖、40 个唯一 XID、原登记/报告/回调、持久 checksum、正确业务和围栏身份。

此前尝试保留原始失败：`132542` 漏配既有的短清理阈值，观察窗口短于默认 DB 清理延迟；`133047` 新测试误要求补偿后清零阶段一历史见证；`133235` 把原重复补偿产生的合法围栏误判为未清理正常 Undo。这些目录不计入最终通过证据。

该现场使用自有 TC，关闭认证并使用夹具 root 账号；不替代生产认证、权限、强制全写入器接入、全部 SQL/值组合或 WP05 总门禁。
