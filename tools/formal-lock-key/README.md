# 原锁键对照与组件验证

完整本轮组件验证：

```powershell
$env:PYTHONUTF8='1'
$env:PYTHONDONTWRITEBYTECODE='1'
$env:TEMP='D:/Temp'
$env:TMP='D:/Temp'
python tools/formal-lock-key/run.py
```

`peer.py`实际重新编译固定原BaseTransactionalExecutor/ConnectionContext/ArrayUtils，1029个自有格式/次序案例调用原方法逐例比较。`native_peer.py`由`live.py`在自有MySQL中调用，核对每配置9个实际日志锁键及4条既有codec日志；追加表在`schema-extra.sql`，16列碰撞插入为Rust/Java共享原字节`collision-insert.sql`，不修改原数据库fixture。

Rust1.99和1.85均由完整验证器执行，8个普通及1个真实目标断言负控，另重跑40文档双向比较。只继承旧38项未变DB场景，不把它们记作新执行。按已有精确缓存和原源构建工作，不安装工具、不切换分支、不调用共享数据库。

[实际范围与证据](../../docs/phase-2/evidence/lock-key-gate/README.md)。完整源JUnit、TC锁覆盖/分支登记、Undo持久化和COMMIT恢复继续开放；默认shared lock暂缓不受此排他锁键增量影响。
