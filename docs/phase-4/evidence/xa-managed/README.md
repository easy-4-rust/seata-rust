# 正式 XA 登记与 RM 接入：选定阶段

最新推进：[Prepared 回滚结束意图与连续响应丢失恢复](PREPARED_ROLLBACK.md)。旧阶段的未知结果记录保留，不回填为当时已通过。

后续已推进 [同事务 Commit 见证恢复](COMMIT_WITNESS.md)；以下描述及 machine ledger 保留第一阶段的历史结果。新写入分支通过 015 见证扩展恢复正向提交；没有见证的旧分支不会被补造终态。

规格事实源：[XA 契约](../../../phase-2/mysql-xa-contract.md)。固定结果及校验目录见 [machine ledger](../../mysql-xa-managed.json)。

受管理入口使用真实 ResourceClient 签发的原 RegisteredBranch，登记成功后持久防重放行，再启动原 XA。正常 Prepare 不发送 AT 的 PhaseOne_Done。两方向回调及重建后的终态读取由生产 BranchResource 完成，不能从恢复缺行推断终态。

最终 MySQL 矩阵每个 Rust 版本运行八项场景：正常两方向和重复回调、登记拒绝及响应丢失、MyISAM 拒绝、绑定方向后的活动作用域封闭、原身份及当前结构保护、连接后持久性配置变化、真实原生 COMMIT OK 截留、早于登记响应的回调。数据库查询独立核对业务、控制行、原生命令和 Prepared。原 Java TC 使用固定原源码构建产物，双 Rust 各完成真实 Commit/Rollback；原始 V1 帧独立核对原 XID、原 long BranchId、XA 类型、资源和可选字段、二阶段与全局终态。

可执行复现入口为 tools/formal-source-junit/xa_managed_mysql.py、xa_managed_java.py；独立检查为 validate_xa_managed.py。工具保留实际编译命令、进程结果、原始包、独立 SQL、源归档和逐文件 SHA-256。生产默认 AT 门槛没有改变。

目录同时保留早期诊断：203254/203446 的 MySQL 元数据字符串解码失败；204720/205553 的故障测试错误地要求响应 ResultCode=Failed，而生产协议可以 ResultCode=Success 携带可重试失败状态；205907 Java 运行后测试格式和 README 换行发生变化，最终源字节检查拒绝该记录。它们不是最终通过证据。最终记录以 machine ledger 指定的目录为准。

**完整 XA Gate 尚未完成。** 原生 COMMIT 已生效但成功响应被截留时，当前实际业务为已提交，控制行为 phase=Commit/completed=0，RM 日志保留 Pending；再次回调不盲发 COMMIT。这证明未知结果没有伪装成功，也实测暴露了终态恢复缺口。仍须实现同 XA 提交见证、持久生产者/结束意图、旧活动会话围栏和完整故障矩阵。早回调缺登记行当前也保持未知，完整悬挂/否定证明继续开放。TCC、Phase 5–7、生产验收及其余总计划未因此关闭。
