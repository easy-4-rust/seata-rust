# MySQL同事务Undo写入验证

沿用[WP05存储契约](../../docs/phase-2/mysql-undo-store-contract.md)。本工具使用自有固定缓存MySQL镜像、随机库与D盘持久数据，封存编译源/依赖和实际执行产物后运行真实测试，再编译固定原Java两个Undo manager来源对象做方法对照。不是完整原Java对象/JUnit迁移或AT提交验收。

```powershell
$env:TEMP='D:/Temp'
$env:TMP='D:/Temp'
$env:PYTHONUTF8='1'
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/formal-undo-store/live.py
python tools/formal-undo-store/live.py --rust 1.85
python tools/formal-undo-store/live.py --library --case connection_proxy::tests::ready_undo_publication_after_deadline_keeps_poison_and_rolls_back_original_transaction
python tools/formal-undo-store/live.py --library --rust 1.85 --case connection_proxy::tests::ready_undo_publication_after_deadline_keeps_poison_and_rolls_back_original_transaction
```

集成测试覆盖独立READ COMMITTED/READ UNCOMMITTED观察与InnoDB实际事务号、原Normal行及时间、已有Normal/GlobalFinished唯一身份保护、空日志/封存、缺失/坏表与双向外键、真实唯一键等待中取消/超时、原80%packet分片重组及context容量、无效身份。Ready期限测试先完成实际MySQL写入，再显式延后方法状态交接，证明最终发布处的deadline门槛；它不是网络丢包或整个调度竞态的完整测试。

`java_peer.py`实际重新编译原AbstractUndoLogManager和MySQLUndoLogManager，使用固定Jackson2.18.3与JDBC8.0.27，实际调用原Normal写入/取子片/删除。Java独立地写相同日志，再把采样的Rust七列原行重插为独立夹具，比较原方法的上下文语义、分片数量、完整原字节和原parser解码身份；每次均回滚。原Root/Sub行XID、状态及时间全部保留，不以根身份修复归档行。错子片XID、错Normal状态的实际副本必须命中目标拒绝。

`--red`仅用于预期失败的开发证据，不是通过验收的模式。脚本只清理已核对ID、镜像与随机owner标签的自有容器，保留D盘数据。所有原源、编译产物、实际JDBC加载来源、运行前后摘要与日志均留证据；失败尝试不会生成成功catalog。

当前显式NONE压缩，UUID_SHORT为Rust适配层分片身份分配，不能声称原UUIDGenerator/全ID策略已迁移。成功flush封存后只允许回滚/丢弃；未提供公开COMMIT，不证明TC登记、业务/Undo原子提交、持久围栏或提交未知恢复。外部TC覆盖问题、完整源测试、WP05/06及后续全部范围继续，共享锁仍暂缓。
