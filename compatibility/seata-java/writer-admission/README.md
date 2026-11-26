# Java AT 写入器准入

面向固定 Seata Java 客户端和 Connector/J 8.0.33 的可选兼容层。将 [JavaAtWriterAdmission.java](src/main/java/io/seatarust/compat/mysql/JavaAtWriterAdmission.java) 编入使用原 `seata-rm-datasource` 与 Connector/J 的 Java 应用。现有业务事务入口负责调用。

1. 停止目标 schema 的写入并排空原 XID。由部署账号应用 [005 绑定表](../../../crates/seata_db_mysql/migrations/005_seata_rs_at_resource_binding.sql) 和 [011 映射列](../../../crates/seata_db_mysql/migrations/011_seata_rs_at_tinyint_profile_binding.sql)。用二进制参数写入 `slot=1` 的原 JDBC 资源 ID。映射值 `1` 表示 Boolean，`0` 表示数字。
2. 从部署配置指定目标物理 MySQL 的 UUID 和 schema。保持 JDBC 路由稳定。写入器账号只读绑定表，由部署账号管理其结构和数据。
3. 每次使用原 `DataSourceProxy` 取得 `ConnectionProxy` 后，先关闭自动提交，再在同一连接调用 `JavaAtWriterAdmission.verify(connection, expectedServerUuid, expectedSchema)`。正常返回后才执行第一条业务 SQL。业务 SQL、Seata 的本地提交和绑定行锁留在同一原事务。校验或后续操作失败时回滚原事务。
4. 变更 UUID、schema、原资源 ID 或映射前再次停写并排空 XID。所有参与同一锁域的写入器必须经过各自的准入入口。Rust 写入器使用 `MySqlOptions::require_at_tinyint_profile_binding()` 读取同一绑定。

兼容层校验当前物理连接及原 Seata 资源标识，只支持当前版本的原绑定表和显式本地事务。数据库权限及应用部署负责限制绕过准入入口的写入。外部写入器完整验收继续由 WP05 跟踪。[验收契约](../../../docs/phase-2/tc-lock-coverage-contract.md#java-写入器事务准入)

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
