# 索引元数据与动态异常传播

状态：本阶段已验证；完整core及全部交付目标继续OPEN，沿用既有WP05/ADR。

- IndexMeta新增16原案例；当前13 IMPLEMENTED/18 MISSING、57原案例MATCH/19 MISSING。
- JDK17原76全部通过；Rust1.99/1.85各434普通测试、89 harness、4doc通过，143 ignored。显式验收包95通过。
- 新144计算观察与原21观察一致；4个JVM枚举identity值仅是输入。9个错误实现实际运行失败、8比较及13JUnit反例拒绝。
- 列表共享、固定/可变列表、可重入时点和原异常身份已验证。Rust回调扩展API显式改为Result，调用点同步。
- 原219运行输入与旧组件保护，依赖锁未变；未重跑数据库。下一步TableMeta及其19原案例。
- 共享锁延期，外部TC风险未接受；TC安全/core/WP05/WP06/V1未完成。

冻结root `run_20261118_042124_511b04b6`；catalog `6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774`，534成员/444输入。

[机器状态](../../wp05-sqlparser-index.json) · [当前对象清单](../../sqlparser_core/对象级对照表.md)
