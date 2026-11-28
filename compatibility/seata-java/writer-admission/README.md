# Java AT 写入器准入

面向固定 Seata Java 客户端和 Connector/J 8.0.33 的可选兼容层。将 [JavaAtWriterAdmission.java](src/main/java/io/seatarust/compat/mysql/JavaAtWriterAdmission.java) 编入使用原 `seata-rm-datasource` 与 Connector/J 的 Java 应用。现有业务事务入口负责调用。

1. 停止目标 schema 的写入并排空原 XID。由部署账号应用 [005 绑定表](../../../crates/seata_db_mysql/migrations/005_seata_rs_at_resource_binding.sql) 和 [011 映射列](../../../crates/seata_db_mysql/migrations/011_seata_rs_at_tinyint_profile_binding.sql)。用二进制参数写入 `slot=1` 的原 JDBC 资源 ID。映射值 `1` 表示 Boolean，`0` 表示数字。
2. 从部署配置指定目标物理 MySQL 的 UUID 和 schema。保持 JDBC 路由稳定。写入器账号只读绑定表，由部署账号管理其结构和数据。
3. 每次使用原 `DataSourceProxy` 取得 `ConnectionProxy` 后，先关闭自动提交，再在同一连接调用 `JavaAtWriterAdmission.verify(connection, expectedServerUuid, expectedSchema)`。正常返回后才执行第一条业务 SQL。业务 SQL、Seata 的本地提交和绑定行锁留在同一原事务。校验或后续操作失败时回滚原事务。
4. 变更 UUID、schema、原资源 ID 或映射前再次停写并排空 XID。所有参与同一锁域的写入器必须经过各自的准入入口。Rust 写入器使用 `MySqlOptions::require_at_tinyint_profile_binding()` 读取同一绑定。

兼容层校验当前物理连接及原 Seata 资源标识，只支持当前版本的原绑定表和显式本地事务。数据库权限及应用部署负责限制绕过准入入口的写入。外部写入器完整验收继续由 WP05 跟踪。[验收契约](../../../docs/phase-2/tc-lock-coverage-contract.md#java-写入器事务准入)

## 可选完整 JDBC 配置准入

原三参数入口仍支持上述 Connector/J 8.0.33 及 005/011 部署。需要与 Rust 完整配置绑定协作时，先排空写入器与未完成分支，由部署账号应用 [021](../../../crates/seata_db_mysql/migrations/021_seata_rs_at_jdbc_profile_binding.sql)，预置 `jdbc_profile`，再调用四参数 `verify(connection, expectedServerUuid, expectedSchema, jdbcProfile)`。编号沿用 Rust 的 1–6；NULL、缺列和不匹配均拒绝。

四参数入口要求实际 Connector/J 8.0.27。奇数码使用 Boolean TINYINT，偶数码使用数字映射。配置 3–6 还要求驱动 `connectionTimeZone=UTC`、实际 MySQL session `time_zone='+00:00'`、JVM 固定零偏移时区、原 Jackson Undo parser 固定零偏移时区。原版本没有 parser 时区 getter，守卫只读其固定版本字段；无法读取即拒绝，不修改进程全局配置。部署方必须在启动阶段配置这些约定，并保持到所有分支及回调结束。

配置 5/6 必须把实际 `ExactTemporalDataSource` 放在原 Seata `DataSourceProxy` 直接内层；配置 1–4 不能使用该装饰器。旧入口无须编译或链接该装饰器。共享绑定行锁和 MDL 仍归首条业务 SQL 前的原事务持有，调用方须在失败时回滚。

[真实数据库探针](../../../docs/phase-2/evidence/java-writer-jdbc-profile/README.md)证明六码绑定、时间与装饰器拒绝及持锁，并验证旧入口。该探针只替换 Seata 数据源身份，未覆盖完整 Java TC/RM 回调；完整写入器门禁仍开放。

后续[原 Java TC/RM 联试](../../../docs/phase-2/evidence/java-writer-jdbc-profile-tc/README.md)补齐新入口的实际代理接入：六码各完成原提交/回滚，精确模式实际补偿 DATE/TIME 普通列与主键。该联试不使用身份替身，保留原 Undo/Jackson 与原 RM；限定在记录的驱动、SQL、值域样本和自有 TC 环境。混合写入器竞争和完整生产准入仍需按 WP05 验收。

## 账号权限验证

固定 MySQL 8.4.13 联试对 Java 写入、Rust 写入以及只读业务查询使用三个独立账号。两个写入账号对 005/011 表仅授予 `SELECT`。部署账号负责 DDL、初始化绑定和后续变更。

| 账号 | 联试授权 |
|---|---|
| Java 写入 | 业务表 `SELECT, UPDATE`，原 Undo 表的读写，005/011 表的 `SELECT` |
| Rust 写入及 RM 恢复 | 业务表 `SELECT, UPDATE`，原 Undo 与恢复表的读写，005/011 表的 `SELECT`。只对业务表、Undo 和恢复表直接授予 `TRIGGER`，直接授予全局 `PROCESS` |
| 只读业务查询 | 业务表 `SELECT` |

该授权覆盖现场用例的 UPDATE、登记和回滚。Rust 仍使用全局 `PROCESS` 核对跨 schema 入向外键。实际业务表权限应按使用的 SQL 类型核对。使用这些凭据的写入应用仍须接入事务准入。

Rust 实验性有变化提交另需通过部署账号应用 006/007，并对 `seata_rs_at_commit_attempt` 授予 `SELECT, INSERT, UPDATE, TRIGGER`。提交尝试读取、写入与成功报告门闩均依赖可见的表级触发器元数据。[006/007 授权契约](../../../docs/phase-2/mysql-first-phase-commit-contract.md#006007-授权检查)。

## Rust 定义者元数据通道

固定 MySQL 8.4 环境可以部署 [012 元数据存储过程](../../../crates/seata_db_mysql/migrations/012_seata_rs_at_constraint_flags.sql)，让 Rust 写入账号使用直接过程 `EXECUTE`。部署管理员创建锁定登录的定义者账号，授予它全局 `REFERENCES`、目标 schema 的 `TRIGGER` 和该过程的 `EXECUTE`。安装 012 时，在目标业务库的 `CREATE PROCEDURE` 明确指定这一账号为 `DEFINER`。运行账号不参与过程和定义者授权维护。停写并排空在途 XID 后，才切换授权。

Rust 会在原连接核对过程定义和定义者返回的授权证据。触发器或者入向与出向外键任一存在即拒绝。定义者权限不足或被部分撤销时也拒绝。写入器继续使用同一 005/011 绑定和原登记身份。
