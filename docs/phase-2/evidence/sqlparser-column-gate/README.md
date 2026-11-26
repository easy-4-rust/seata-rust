# 列元数据与无损UTF-16依赖阶段

状态：本阶段已验证，完整core迁移仍OPEN；唯一规格事实源仍为既有WP05/ADR。

- 新增ColumnMeta完整20字段及共享身份，修复两个旧对象的UTF16缺口。当前12 IMPLEMENTED／19 MISSING。
- 固定原Java76测试新跑通过；累计41原案例双Rust差分MATCH，剩35案例待迁移。另21补充观察独立计数。
- Rust1.99/1.85各406普通测试、85 harness、4doc通过，143 ignored。5组错误实现共8行为失败、8比较及13JUnit反例拒绝。
- Debug持锁回调与equals/hash/toString快照顺序两项P2有真实RED并修复；源码独立复核通过。
- 606行ColumnMeta仅对应一个Java对象及私有状态，内聚例外经审查；19对象缺失继续阻断完整模块。
- 无数据库现场重跑，无依赖锁升级；共享锁延期，TC/WP05/WP06/V1未完成。

最终root `run_20261118_030159_4362448c`；catalog `1e54b372ce3454028d650bb7d46e72f25a3a4d42c724441d1ec04756ace213da`，483成员／422输入。

[机器状态](../../wp05-sqlparser-column.json) · [当前迁移清单](../../sqlparser_core/对象级对照表.md)
