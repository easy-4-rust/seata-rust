# WP05：完整 JDBC 表示的登记前准入

延续资源 ID / TINYINT 绑定与时间类型契约。当前只核对 tinyint1_is_bit，配置 1/3/5 及 2/4/6 各自布尔位相同；完整表示混用仍可能通过该检查。本增量实现完整配置绑定的可选严格入口，不改变既有默认行为与编号。

部署方排空旧写入器和未完成分支后，应用 021 迁移，在 slot=1 预置 jdbc_profile（稳定码 1–6），并保持 011 的 TINYINT 位一致。迁移不自动填写或推断现有行。Rust 通过 MySqlOptions::require_at_jdbc_profile_binding 显式启用；业务事务池或准备管理器任一端启用都须检查，不能由另一端较宽配置取消。

登记前在原业务事务中共享锁定原资源绑定行，核对 InnoDB 基表/当前引擎、精确资源 ID、TINYINT 位和完整配置码。NULL、缺列、不同编号或不兼容映射失败后原本地事务回滚，BranchRegister 不发送。共享行锁与 MDL 随原事务保持到实际提交/回滚，独立连接的绑定更新必须等待锁释放。旧入口不读取新列；旧 schema 与编号保持原兼容范围。

验收包括默认/严格选项和合并要求测试、真实 MySQL 的完整配置码及行锁验证，以及原 Java TC 下事务池/管理器两条入口的拒绝与正式提交/回滚。Java 与其他写入器仍须使用相同约定；本守卫不为未经接入的写入器签发能力。WP05 的所有写入器全局锁协作、账号权限、生产部署准入及完整门禁继续开放。

## 2026-11-28 验证状态

Rust 可选严格入口已实现，并取得[真实双 Rust/原 TC 现场](evidence/mysql-jdbc-profile-admission/README.md)：两个启用位置各自生效，八笔全局事务、40 次错误配置拒绝；每个版本的六码矩阵为 6 接受/30 拒绝，旧表、缺列、NULL、未知码与实际共享锁等待均核验。218 次证据削弱被拒绝。此状态关闭 Rust 表示绑定守卫增量，不关闭所有写入器的全局锁、账号权限和生产准入门禁。

## Java 写入器完整配置入口

沿用 `JavaAtWriterAdmission` 原显式事务调用点，增加四参数 `verify(connection, uuid, schema, jdbcProfile)`；三参数入口保持 005/011 兼容行为。新入口只接受稳定码 1–6，核对实际 Connector/J 8.0.27、TINYINT 映射和 021 完整码。配置 3–6 要求 JVM、原 Jackson Undo 时区、实际驱动时区及 MySQL session 显式 UTC；配置 5–6 要求实际内层数据源是精确 DATE/TIME 装饰器，1–4 不允许该装饰器。仅有调用方声明不足以通过。

检查、共享行锁与 MDL 留在首条业务 SQL 前的原连接和事务；失败由调用方回滚，不改原 Undo、配置默认值或编号。此入口仍为协作式应用接入，不能阻止任意应用绕过，也不直接证明全部 Java RM 时间类型补偿兼容。测试须分别标明真实数据库检查、替身身份边界与完整原 TC/RM 现场的证据范围。

实现已通过[真实 MySQL 探针](evidence/java-writer-jdbc-profile/README.md)：6 接受、69 拒绝、6 次绑定更新超时与回滚后释放；原三参数入口在 NULL/缺列和 Connector/J 8.0.33 下保持兼容。原 `ConnectionProxy` 与驱动实际执行，`DataSourceProxy` 身份使用替身。完整 Java TC/RM 集成和所有写入器门禁继续开放。

后续已补[完整原 Java TC/RM 集成](evidence/java-writer-jdbc-profile-tc/README.md)：六码各自提交和回滚，共 12 笔全局事务、60 次登记前错误绑定拒绝；不再替换数据源身份。配置 5/6 的七条业务 SQL 实际生成原 Undo，原 RM 提交/补偿、DATE/TIME 普通列与主键、数据库终态和原回调帧均独立核验。该增量关闭新入口的选定原 Java TC/RM 链路缺口；所有写入器的混合竞争、权限/认证、全部 SQL 和生产门禁仍开放。

新增[Java 持锁、双 Rust 竞争契约](mysql-exact-temporal-mixed-lock-contract.md)：配置 5/6 的 DATE/TIME 原键在 Java 持锁时取得 16 次原 TC 冲突拒绝，Java 释放后 16 次 Rust 准备登记成功；双工具链、Java 提交/回滚均覆盖。未提交分支回滚后保留原 `GlobalFinished` 围栏，独立观察按原身份核对。反向持锁与全部写入器门禁继续开放。

后续已补[反向 Rust 已提交持锁、原 Java 竞争](evidence/exact-temporal-rust-owner-java/README.md)：双版本、配置 5/6、Rust 提交/回滚八组取得 40 笔实际全局事务；Java 16 次原 TC 冲突拒绝、16 次释放后成功登记及原 RM 补偿。阶段一原 Undo checksum、历史见证和保留围栏分别核验。选定精确键的双向竞争链路已验证；完整 SQL、权限/认证及生产准入仍开放。
