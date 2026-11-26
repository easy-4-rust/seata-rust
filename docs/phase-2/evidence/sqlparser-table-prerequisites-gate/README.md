# TableMeta主键集合依赖

沿用既有WP05/ADR及完整31对象/76案例规格。此依赖阶段已验证，TableMeta本体仍待实现。

- 固定JDK17比较器及不可变主键树已实现；发现并修复非传递比较器下排序数组改变原树行为的P2，有真实RED。
- 新23,370计算观察MATCH；2,799字符映射仅为输入。原57源案例及165补充观察回归MATCH，原19案例/18对象仍缺失。
- Rust1.99/1.85各437普通测试、90 harness、4doc通过，143 ignored；显式验收包98通过。
- 7错误实现真实运行失败，8比较反例拒绝；219原运行输入和5个实际JDK文件绑定。原JUnit76为继承证据。
- 无新增依赖或DB现场认证；共享锁延期，外部TC风险未接受，完整core/WP05/WP06/V1及TC安全未完成。

root `run_20261118_044535_5f39165d`；catalog `9df62e28460c66b2aae94071d417f2f10ce3c2a46eedf02af237b3daeb6845bb`，537成员/468输入。

[机器状态](../../wp05-sqlparser-table-prerequisites.json) · [对象清单](../../sqlparser_core/对象级对照表.md)
