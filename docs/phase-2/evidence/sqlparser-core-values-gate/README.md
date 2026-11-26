# SQL parser core 基础对象迁移

状态：首个依赖阶段已验证，完整模块未完成。现有 WP05/ADR 是唯一规格事实源。

- Java固定 `cba73e180fd8896a99df6415b859d275122feaa7`，Rust基线 `130ad634279cef10cd2998fc3bfae82996224d66`，未提交既有工作保留。
- 新增11对象；9 IMPLEMENTED、2 PARTIAL、20 MISSING；完整分母31对象／76源案例。
- 原JUnit76通过；本批23原案例双Rust差分 MATCH，完整88常量匹配。
- Rust1.99/1.85各369普通测试、81 harness、4文档通过；143 ignored，数据库现场本轮未重跑。
- 6运行时错误实现、4差分反例、13原JUnit事件反例均拒绝。初始编译RED不冒充行为RED。
- Java孤立代理码元String域未覆盖，两个字符串对象保持PARTIAL；共享锁延期、TC安全、WP05/WP06/V1仍未完成。

[机器状态](../../wp05-sqlparser-core-values.json) · [完整迁移路线](../../sqlparser_core/迁移路线图.md)

最终root：`run_20261118_022222_bb4f2677`；catalog `b0056e0927ef6a565c07f7c39ee49b2cc7fed58ad77bc08bf47bec7acec989cd`，441成员。
执行命令与退出码：[完整记录](run_20261118_022222_bb4f2677/verification/rust-commands.json)。
历史MSRV观察器Clippy和首次保守mutation判定失败保留为诊断，只有最终成功run被提升。
