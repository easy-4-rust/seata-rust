# seata-orm-rbatis

正式 rbatis `Executor` 适配，独占持有同一个 MySQL AT 本地事务。`RbatisMySqlExecutor::new(local)` 接收原事务，原生 CRUD 和 SQL 宏执行后，通过 `into_transaction()` 交还原事务，继续原 manager.prepare_transaction/锁/提交授权/回调/恢复流程。没有独立 COMMIT、Clone 或原始连接访问入口；未交还即 Drop 沿用原事务回滚/关闭语义。

所有实际 exec/query 都复用正式 SQL/元数据/预算准入。支持原生整数、rbdc Bytes、Decimal、JSON 原文本及已认证日历输入；拒绝浮点到 DECIMAL、未知扩展、控制语句、错误参数数量与超预算。JSON 结果为原 JSON 文本，可用 String 模型保持完整字节/数字语义；DATETIME 为日历文本，TIMESTAMP 为明确 +00:00 的微秒文本。DateTime 输入遵循原 rbdc MySQL 编码器的日历字段语义；毫秒 Timestamp 扩展保持原单位。

原生读取和 COUNT 使用原 RR 事务，不创建 rbatis 业务池。元数据引擎初始无池、无默认日志；配置了池或拦截器时，该次执行明确拒绝。完整插件、分页接口与通用 ORM 类型兼容仍须继续交付，不能把首期单表准入冒充整个 rbatis 生态。

根工作区明确集成 rbatis/rbdc/scraper/宏驱动的 Rust 1.85 维护补丁及 time 的已验证安全回移。rbdc 的 lru 使用官方修复版 0.18.2；uuid 精确约束为已验证的 1.26.1，避免未声明的传递 MSRV 变化。发布消费者必须显式集成维护补丁，根 patch 不会自动继承。

现场使用 `tools/formal-source-junit/native_rbatis_mysql.py` 和独立 verifier，实际检查原 TC 双版本 CRUD 提交/补偿、原 Java Undo parser、SHA/锁/回调，以及原生值/快照/预算/取消/丢弃。总验收按 [正式契约](../../docs/phase-2/rbatis-mysql-contract.md) 与原 Phase 0–7 计划继续推进。
