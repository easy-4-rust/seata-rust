# 原 Java TC/RM：完整 JDBC 准入与时间类型补偿

权威包：[20261128_130125_e7002ecc](20261128_130125_e7002ecc/result.json)。catalog SHA-256：`148d183d72bc36cbe698eeaee866e773beb8b4e0ab3627f35a8ec9d7456a2d01`。

这是独立 Java 矩阵的归档基线。后续扩展夹具的当前完整矩阵见下方“Java 持锁、双 Rust 竞争”，同时重新运行上述 Java 基线。

使用固定原 Seata TC JAR、原 Java TM/RM/DataSourceProxy/UndoLogManager、Connector/J 8.0.27、Jackson 2.18.3 和 MySQL 8.4.13。Java actor 只组织业务、调用正式准入入口并等待外部观察；无客户端替身、TC/数据库替身、协调器或原 parser 补丁。透明代理只记录实际原始帧。

六码各执行 commit/rollback，共 12 个不同全局事务、12 个不同分支。每个参与者先尝试另外五码绑定，60 次均在 Begin/BranchRegister 前拒绝，业务和四类事务表不变。随后正确入口在原事务内持有绑定共享锁，独立连接 UPDATE 实际返回 1205；阶段一提交后锁释放。

配置 1–4 每组执行一条 INT UPDATE。配置 5/6 每组执行七条真实 DML：INT UPDATE、DATE/TIME 普通列 UPDATE/INSERT/DELETE、DATE/TIME 主键 UPDATE 和 DATE 主键 INSERT。现场包含负一微秒 TIME、跨日与负跨日 TIME、正负 838:59:59、DATE 历法缺口和 1000/9999 边界。原 Undo 保留 JDBC 91/92 和自然 String，继续由原 Jackson parser 编码；不引入新的 Undo 格式。

阶段一独立查询原 branch_table/lock_table，精确核对原 XID、branch ID、资源和每个主键。四组精确模式均实际完成原 RM 回调：commit 保留阶段一数据，rollback 恢复原业务字节；原 TC 的 global/branch/lock 和 Undo 最终都清空。回调状态按固定原源码核验为 PhaseTwo_Committed=5、PhaseTwo_Rollbacked=8，并按物理连接和请求 ID 配对原响应。

此现场使用自有 loopback TC，认证关闭、数据库使用夹具 root 账号；不证明生产认证、权限或强制全写入器接入。精确模式证明上述 SQL、值和 Jackson 路径，不扩展为 Fastjson、所有 JDBC 驱动或所有 ORM/SQL 组合的认证。

```powershell
python -B tools/formal-source-junit/java_writer_jdbc_profile_mixed.py
python -B tools/formal-source-junit/validate_java_writer_jdbc_profile_mixed.py docs/phase-2/evidence/java-writer-jdbc-profile-tc/20261128_131452_bf5fab52 --full
```

独立校验器复核完整矩阵、原始帧、登记与回调配对、原 Undo、精确业务结果、原数据库锁记录、源码快照、依赖哈希、编译类和实际类加载来源。`source/`、`classes/`、依赖清单、客户端/TC 命令及原始日志均随包保存。

此前目录保留原始尝试：`125547` 是写命令清单时误用只允许首次写入的辅助函数；`125603` 未为原 TC 加载外部 JDBC 驱动；`125720` 是观察器未展开原 Jackson ArrayList 包装；`125837` 已完成真实补偿，但观察器使用了错误的分支状态常量。它们不作为最终通过证据，最终包未修改原 TC 或 RM 来通过断言。

## Java 持锁、双 Rust 竞争

当前完整包：[20261128_131452_bf5fab52](20261128_131452_bf5fab52/result.json)，catalog SHA-256：`ffeee9ef7bd0ff281da6723f0f659adba1695fdf5df4564b1b892f26c262a6e7`。[对应契约](../../mysql-exact-temporal-mixed-lock-contract.md)。

上述六码 Java 提交/回滚基线重新通过。四组精确模式 Java 持锁期间，Rust 1.99.0 和 1.85.0 分别用正式类型化 DATE/TIME 参数执行 UPDATE、准备和登记。原 TC 实际拒绝 16 次锁竞争，Java 原分支、七条锁记录和阶段一业务均保持不变。Java 完成提交/回滚后，16 次相同资源/键的 Rust 准备取得原 TC 成功登记；独立数据库逐一观察到对应 XID、branch ID 和精确主键。准备未提交，独立连接仍只看到 Java 最终结果。

共 44 笔全局事务（12 Java、32 Rust）。Rust 的 16 个已登记但未提交的分支均撤销本地准备，并完成原 TC 的全局回滚回调。原 `GlobalFinished` 围栏各自保留，两个版本结束后每组四个围栏，正常 Undo 为零，TC global/branch/lock 为零。证据保存每次围栏的原 XID/branch ID、状态、原字节和上下文。下一组测试仅在两端客户端退出后审计并重置这些已结束的自有夹具围栏；最后一组围栏保留在停止的自有 MySQL 数据目录，未借助删除来声称系统自动清空 Undo。

`rust-binaries.json` 绑定实际执行的双版本二进制；Cargo 构建记录、rustc 版本、全 Rust 工作区源码快照、各原生客户端日志、原始帧、拒绝/登记收据和独立 SQL 输出均随包保存。独立校验器同时检查 Java 基线及混合请求/响应的物理连接、请求 ID、XID、branch ID、锁键和围栏。

`131037` 是把原 `GlobalFinished` 围栏误判为残留 Undo 的失败观察；`131248` 是修正后单组双 Rust 回滚方向的已通过原型，不代替当前完整矩阵。仅新增 Rust 联试目标与夹具，本轮没有改变 Rust 生产组件；两个版本对新增目标的严格 Clippy 均通过。反向的 Rust 已提交持锁、Java 竞争，以及完整生产准入继续开放。
