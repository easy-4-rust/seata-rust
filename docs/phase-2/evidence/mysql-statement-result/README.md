# 原业务语句回执与 SeaORM 传播

对应 [SeaORM 适配契约](../../seaorm-mysql-contract.md)的原语句回执扩展。新增公开不可变 `MySqlStatementResult`，保留同一业务 DML 的原始 u64 `rows_affected` / `last_insert_id`，通过 INSERT/UPDATE/DELETE 镜像准备返回正式本地事务与 SeaORM 适配器。新增 `execute_with_result`；旧 `execute` 保持返回行数。回执直接取自原 SQLx 执行结果，完成镜像/Undo 预算校验后才发布，没有增加执行、连接或提交入口，也不存储可串到下一次调用的缓存结果。两个接口共用一个指标守卫。

权威现场为 [20261128_163855_5075e62d](../seaorm-mysql/20261128_163855_5075e62d/result.json)，catalog SHA-256：`1e495c7b08a082fd68b9d8816aea66957a0cf720c5a5e2b4cb3bf4b7506050df`，共 1388 个哈希成员。执行输入包括正式 crate、SQLx 维护补丁、锁文件、契约、运行器及独立验收器，执行前后无源码漂移。

Rust 1.99 / 1.85 在真实 MySQL 上分别验证底层原回调的 ID `9223372036854775811`，独立 SELECT 得到同值；随后将会话 LAST_INSERT_ID 改为 7，原回执仍保留大整数。批量回执只返回原服务端首 ID 与计数，不推算其他编号；UPDATE/DELETE 的原 OK ID 为零，重复键返回错误，显式回滚后临时表业务行数为零。该临时自增表测试只检验 StatementCallback 协议回执，**没有绕过正式 AT 元数据准入并声称自增表已支持**。

真实 SeaORM Entity 的 INSERT/UPDATE/DELETE 在未修改原 Java TC 下各做提交/补偿，双版本共十二笔；原始非自增语句 OK ID 均为零，影响行数为一。独立观察验证完整业务字段、精确十进制/二进制/中文 JSON/NULL、原锁和登记、原子 Undo/SHA/历史见证、回调与终态；原 Java parser 重读全部十二份 Undo。双版本全工作区各 840 passed / 0 failed / 416 ignored；ORM 组件各 7 passed / 1 ignored，受影响 MySQL/ORM all-target/all-feature 严格 Clippy 均通过。忽略用例没有计为通过；底层回执用例另外以 --ignored --exact 实际执行。

输入先新增回执调用，编译按预期因缺少 `execute_with_result` 失败，日志为现场的 `red-missing-api.log`。首轮 163246 的测试夹具借用连接不满足原拥有式静态事务边界，编译失败且未启动 MySQL，保留 incomplete_evidence。163318、163509 为中间原生运行；格式恢复和旧验收器更新后，以 163855 的最终源码字节及完整回归日志为权威，未用中间轮替代最终验证。

本轮无新依赖；正常构建图仍无 rsa/time/SQLite，并保留 aws-lc-rs 维护补丁。完整 lock 审计仍退出 1，保留 time 与 libsqlite3-sys 两项既有告警，无 ignore；不是生产安全验收通过。

复核命令：

```powershell
python -B tools/formal-source-junit/validate_seaorm_mysql.py docs/phase-2/evidence/seaorm-mysql/20261128_163855_5075e62d --full
```

同目录封存全部组件/全工作区/Clippy 原始日志、实际 argv、依赖树和原审计结果，以及生成回归和汇总的脚本。原生复跑命令仍为 `python -B tools/formal-source-junit/native_seaorm_mysql.py`；组件与审计汇总须另按封存的命令执行后才作完整验收。

未完成：正式 AT 自增列准入、缺省/NULL/零值与 SQL mode 的生成主键镜像定位、多行生成键支持、原生 ConnectionTrait、读写模型返回及完整 ORM。后续继续这些原范围，保持 Java Undo 格式、旧记录兼容及原提交/未知/恢复边界。WP05/WP07、完整 V1 与 Phase 5–7 未因此关闭。

后续 [正式单行自增镜像](../mysql-auto-increment/README.md)已完成已列明范围的实现和 52 笔双版本原 TC 验证；缺省/NULL/零和原回执定位不再是完全缺失项。显式自增键的服务端 OK ID 可能为显式值，原接口持续原样传播。完整 ORM 与生成键批量仍开放。
