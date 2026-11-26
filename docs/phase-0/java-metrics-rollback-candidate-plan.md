# Java 指标候选：真实 FILE 回滚恢复验证

延续 ADR-0008、REC-TIMEOUT-008/009、JAVA-METRICS-RETRY-001 和现有指标生命周期计划。本次只扩展独立0004候选的真实自然17验证，不改变正式需求、WP00前置或原参考实现，不覆盖已封存文件。

复用已封存FILE自然17的原Go业务、代理和完整验证器，以及0004已通过77项Java测试的相同MetricsSubscriber字节。新只读控制入口读取实际CompactRegistry的回滚指标和原事件，不修改producer、事件、事务、计数或采样窗口。

验收行为：

1. 原TM只请求一次回滚，实际回滚请求被阻断后由TC自然进入17；原Begin与有结束时间的RollbackRetryTimeout使active归零，failed和two_phase_timeout各一次，失败Timer恰有一个真实耗时样本。
2. TC、SDK、业务数据库重启，使用上一阶段停止后的相同FILE日志；每个新TC在SDK启动前有独立零基线。
3. 人工恢复产生endTime为空的原17 doing事件。实际补偿执行、余额恢复、Undo清空，成功回复被代理丢弃，TC保留原分支和锁；候选跳过该doing事件的完成分派，不能扣active、增加failed或补零Timer。
4. 再次重启、同分支重放保持业务幂等，原会话最终移除。真实AfterRollbacked完成事件使对应Counter、Summary、Timer各一次；active与failed仍为零，耗时严格对应事件。
5. 原事件需绑定XID、名称、标签、时间及重试标记。两次人工恢复的指标快照必须在同TC候选处理该空结束时间事件的DEBUG标记之后读取，保存标记行及快照摘要。
6. 完整复核SQL、Undo、防悬挂记录、原始RPC、控制台、原生查询、两次FILE恢复输入、类加载来源、依赖与源文件摘要、进程和容器清理。负向测试必须拒绝错误状态、缺失事件、虚构计数/耗时或未等处理结束的快照。

这是FILE自然17范围证据；不代表原参考问题已修复，也不包括全重启指标重建、去重、Exporter抓取、RAFT/所有SQL/崩溃矩阵或正式Rust交付。若出现额外生产缺陷，应另立候选，不能削弱验收断言。
