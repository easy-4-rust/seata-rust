# 首期正式 rbatis 原事务适配

权威现场：[20261128_202109_4a57fd2e](20261128_202109_4a57fd2e/result.json)，对应 [正式接入契约](../../rbatis-mysql-contract.md) 与 [正式 crate](../../../../crates/seata_orm_rbatis/README.md)。

`RbatisMySqlExecutor` 实现真实 Executor/RBatisRef，独占持有原 `MySqlLocalTransaction`，原生 CRUD 和宏经密封 SQL/精确值准入后执行；消费执行器交还同一事务进入正式准备/TC 锁/提交授权/回调/恢复流程。没有独立 COMMIT 或连接/池访问授权。核心不依赖 ORM，现有 SeaORM 保留。

双 Rust 1.99.0/1.85.0、原 TC、真实 MySQL 下的 INSERT/UPDATE/DELETE 提交及补偿共 **12 笔全局事务**通过。生成 ID 101 来自原语句回执，UPDATE/DELETE 原回执 ID 为 0；原生模型读自己的未提交值与 COUNT 通过。精确 DECIMAL/scale 4、二进制、中文 JSON 文本和 NULL 原字段、原子 Undo SHA、原 Java parser、真实锁键和回调帧均独立核验。

普通只读、DATE/TIME/DATETIME/明确 UTC TIMESTAMP、正负 TIME、RR 外部提交隔离通过。九项 SQL/分页拒绝、两项配置拒绝、深容器未轮询 Future 的安全丢弃、大字段/1025 行拒绝、真实 MDL 取消后的毒化和新作用域恢复通过。未提交写入后 Drop 实际回滚并释放 MDL，以随后 DDL 成功和外部原值证明。早期实际 TIMESTAMP 解码及 TIME 符号失败的日志、输入与源码覆盖保存在 `red-executions`，不是通过删去用例通过。

双版本全工作区各 **861 passed / 420 ignored**，AT/MySQL/SeaORM/rbatis 四包严格 Clippy 通过；独立 rbatis 兼容工程各九项实际 CRUD/宏/缓存 API 检查和严格 Clippy 通过。lru 精确升级官方 0.18.2，两个旧 unsound 警告消失，无 ignore；根审计仍保留 RUSTSEC-2026-0459（正常图未启用的 SQLite）。维护 time 采用已有源码回移，path 依赖的扫描结果不能替代源码证明，见 [time 现场](../time-rfc2822/README.md)。

原生与回归执行的源码起止摘要一致；最终 1816 个证据成员的 catalog SHA-256 为 `b80a3bda8104f38af1cae9c37b6b2c55e39f147b438fb50d494a29b904f9b804`。

复验：`python -B tools/formal-source-junit/validate_rbatis_regression.py docs/phase-2/evidence/rbatis-mysql/20261128_202109_4a57fd2e`。原生重跑使用 `native_rbatis_mysql.py`。

首期原事务适配已取得实施和现场证据；完整插件/分页接口、关联/迁移和更多类型生态兼容、下游维护补丁发布、完整 SQL/全部写入器、生产准入及 Phase 5–7 继续开放。JSON 模型使用原文本，首期不运行用户拦截器；有变化 COMMIT 仍需显式 experimental feature，不关闭完整 V1/生产门禁。
