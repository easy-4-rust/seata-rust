# 原生单表计数、分页与只读 LIKE

对应 [分页契约](../../seaorm-pagination-contract.md)，延续已交付的借用式连接。AT 层新增独立密封 MySqlCountRecognizer，MySQL 层新增同原事务的 query_count；普通读取与计数共用同一参数预算、元数据、RR、超时/取消/poison 边界。SeaORM 只负责计划分派、绑定与原生结果转换。核心未绑定 ORM；rbatis 的实际适配仍未实施或认证，不能把本增量描述为已接入 rbatis。

COUNT 只允许不带修饰的原生内建 COUNT(*)，可用于单表或一层已完整核对的普通 SELECT 派生表。统计 SQL 不增加普通读取的 1025 行探测 LIMIT，保留原派生表显式 LIMIT/OFFSET。COUNT 客户端只接收一个非空非负原生整数行；保留 MySQL SIGNED BIGINT 类型，不转浮点、不伪造总数。拒绝带 schema/引号的同名函数、非星号参数、DISTINCT/FILTER/窗口、通用函数、组查询、集合、嵌套派生表、外层派生表过滤/分页、锁定读、变量/文件输出和多语句。

LIKE/NOT LIKE 与单字符 ESCAPE 仅扩展只读谓词；绑定顺序、NULL、MySQL 排序规则及原会话不改写。DML 的 LIKE 准入没有扩大。行/分页预算仍为单次最多 1024 行、OFFSET 不超过 1000000，计数的服务端扫描仍受原执行时限约束。

权威现场：[20261128_175609_4a2479bb](20261128_175609_4a2479bb/result.json)，最终 catalog SHA-256：`35a187abf7436d2d16578723f09ac0e7dc3e437302505430871fd62c5a711883`，1410 个哈希成员。双 Rust 1.99/1.85、真实 MySQL、未修改原 Java TC/原 Undo parser。真实实体插入/更新/删除各做提交和补偿，共十二笔实际全局事务；模型未提交变化的原生计数分别与本事务完整行数一致。独立业务/锁/原登记/报告/回调、原子 Undo/SHA/见证、终态及十二份原 Java Undo 重编码均核验通过。

每个流另用真实 Entity/Paginator 验证 3006 行，超过 1025 探测边界：num_items=3006、num_pages=3、num_items_and_pages 一致；fetch_page、fetch、fetch_and_next、cur_page、分页 into_stream 均实际运行。三个非空页为 1007/1007/992 行，ID 边界连续且不重复/遗漏；分页流总计 3006，最后/越界页与空集正确。显式派生表 LIMIT 2 OFFSET 1 的统计返回 2，未被去除。

contains/starts_with/ends_with、NOT LIKE、中文单字符通配符、NULL 与显式 ESCAPE 的百分号/下划线均通过。外部提交后原 RR 仍统计 3006，新事务统计 3007；自己未提交的插入可统计 3007，外部仍为 3006，回滚后恢复。大页/大偏移及五项非法统计形状被拒绝。此前原生列序/时间、九项副作用拒绝、超大字段/行预算及真实 MDL 取消检查在本现场也重跑通过。

双版本 workspace/all-features 各 **853 passed / 0 failed / 419 ignored**；AT/MySQL/SeaORM all-target/all-feature 严格 Clippy 通过。忽略项未计通过，原生测试另用 --ignored --exact 执行。COUNT 识别测试先因 API 缺失编译失败；[175407](20261128_175407_dca921bf/result.json) 在真实 COUNT 上暴露错误的无符号类型检查，保留 incomplete_evidence。修订实现保留原生有符号类型并检查非负，再以全新现场通过；没有改写服务端结果或放宽预期数字。

```powershell
python -B tools/formal-source-junit/native_seaorm_pagination.py
python -B tools/formal-source-junit/validate_seaorm_pagination.py docs/phase-2/evidence/seaorm-pagination/20261128_175609_4a2479bb --full
python -B tools/formal-source-junit/validate_seaorm_pagination_regression.py docs/phase-2/evidence/seaorm-pagination/20261128_175609_4a2479bb
```

现场封存完整运行输入、原二进制哈希、协议帧/SQL/Undo/Java 重编码、分页/读取检查、回归/依赖树/审计及生成脚本。复跑新现场须按原命令另执行完整回归并汇总。默认适配没有启用 ORM 驱动，MySQL 核心无 SeaORM 依赖；仅新增已锁定 futures-util 的测试直接依赖，没有升级包版本。完整 lock 审计仍退出 1、保留 time/SQLite 两项告警且无 ignore；正常图未启用它们和 rsa。

此增量仅认证已支持单表读取矩阵上的原生分页；通用聚合、DISTINCT/GROUP BY、关联、DML LIKE、数据库游标 StreamTrait、嵌套事务、迁移/2.x、更多适配器、全部写入器和生产门禁继续开放。没有修改上游 Paginator 的输入/整数转换实现，也不把回环 root/关闭 TC 认证的现场算作生产权限证明；WP05/WP07 和 Phase 5–7 继续推进。
