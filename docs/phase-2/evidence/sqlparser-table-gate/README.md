# TableMeta首期实现及原案例差分

原76案例双工具链全部MATCH；源对象仍13 IMPLEMENTED、1 PARTIAL、17 MISSING。TableMeta不宣称完整迁移。

- 已实现共享元数据、显式Locale、主键/更新列及原equals/hashCode行为；283条主键顺序和12条表边界计算结果MATCH。
- 原JUnit76本轮新运行；Rust1.99/1.85各471普通测试/95 harness/4doc通过，143 ignored。显式验收包131通过、0忽略。
- 6错误变体运行时失败，12比较反例拒绝；继承Locale存活变体未分类，不计拒绝且未证明等价。
- 默认Locale引导、转义/SPI、SqlServer桥和完整Map视图未完成；全部core/WP05/WP06/V1与TC安全保持OPEN。

root `run_20261118_105502_3c628c06`；catalog `84080e6737b775669c064aed2d792a191ba16da54731125976114792e779d802`；642成员/523输入。

[机器状态](../../wp05-sqlparser-table.json) · [完整对象清单](../../sqlparser_core/对象级对照表.md)
