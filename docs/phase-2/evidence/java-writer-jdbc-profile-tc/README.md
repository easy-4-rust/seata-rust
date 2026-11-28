# 原 Java TC/RM：完整 JDBC 准入与时间类型补偿

权威包：[20261128_130125_e7002ecc](20261128_130125_e7002ecc/result.json)。catalog SHA-256：`148d183d72bc36cbe698eeaee866e773beb8b4e0ab3627f35a8ec9d7456a2d01`。

使用固定原 Seata TC JAR、原 Java TM/RM/DataSourceProxy/UndoLogManager、Connector/J 8.0.27、Jackson 2.18.3 和 MySQL 8.4.13。Java actor 只组织业务、调用正式准入入口并等待外部观察；无客户端替身、TC/数据库替身、协调器或原 parser 补丁。透明代理只记录实际原始帧。

六码各执行 commit/rollback，共 12 个不同全局事务、12 个不同分支。每个参与者先尝试另外五码绑定，60 次均在 Begin/BranchRegister 前拒绝，业务和四类事务表不变。随后正确入口在原事务内持有绑定共享锁，独立连接 UPDATE 实际返回 1205；阶段一提交后锁释放。

配置 1–4 每组执行一条 INT UPDATE。配置 5/6 每组执行七条真实 DML：INT UPDATE、DATE/TIME 普通列 UPDATE/INSERT/DELETE、DATE/TIME 主键 UPDATE 和 DATE 主键 INSERT。现场包含负一微秒 TIME、跨日与负跨日 TIME、正负 838:59:59、DATE 历法缺口和 1000/9999 边界。原 Undo 保留 JDBC 91/92 和自然 String，继续由原 Jackson parser 编码；不引入新的 Undo 格式。

阶段一独立查询原 branch_table/lock_table，精确核对原 XID、branch ID、资源和每个主键。四组精确模式均实际完成原 RM 回调：commit 保留阶段一数据，rollback 恢复原业务字节；原 TC 的 global/branch/lock 和 Undo 最终都清空。回调状态按固定原源码核验为 PhaseTwo_Committed=5、PhaseTwo_Rollbacked=8，并按物理连接和请求 ID 配对原响应。

此现场使用自有 loopback TC，认证关闭、数据库使用夹具 root 账号；不证明生产认证、权限或强制全写入器接入。精确模式证明上述 SQL、值和 Jackson 路径，不扩展为 Fastjson、所有 JDBC 驱动或所有 ORM/SQL 组合的认证。

```powershell
python -B tools/formal-source-junit/java_writer_jdbc_profile_tc.py
python -B tools/formal-source-junit/validate_java_writer_jdbc_profile_tc.py docs/phase-2/evidence/java-writer-jdbc-profile-tc/20261128_130125_e7002ecc --full
```

独立校验器复核完整矩阵、原始帧、登记与回调配对、原 Undo、精确业务结果、原数据库锁记录、源码快照、依赖哈希、编译类和实际类加载来源。`source/`、`classes/`、依赖清单、客户端/TC 命令及原始日志均随包保存。

此前目录保留原始尝试：`125547` 是写命令清单时误用只允许首次写入的辅助函数；`125603` 未为原 TC 加载外部 JDBC 驱动；`125720` 是观察器未展开原 Jackson ArrayList 包装；`125837` 已完成真实补偿，但观察器使用了错误的分支状态常量。它们不作为最终通过证据，最终包未修改原 TC 或 RM 来通过断言。
