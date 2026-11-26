# 原身份防悬挂组件资格验证

沿用 `docs/phase-2/mysql-undo-fence-contract.md`。输入清单必须明确引用已结束的证据目录，不按时间或目录名推断通过；最终结果以父包 `result.json` 与 `catalog.json` 为准。

每个 Rust 版本需要九项围栏场景、三种独立原始 MySQL 包故障、一个真实旧 RR 快照测试、十项同事务 Undo 写入回归及一个原 Ready 期限测试。原 Java 的两种 guard 对照与四种存储对照分别保留实际源编译、类加载及 JDBC 来源绑定。其他三十九项数据库场景继承上一组件的限定结论，不计作本次执行。

父包重新解码归档的 PREPARE、参数包、COMMIT 和 OK；核对原身份、物理连接、处理顺序，并对实际证据副本做负向验证。随后执行两个版本的工作区全部测试，归档实际测试程序和 RM 恢复子进程资料；核对既有依赖和未修改的方法体；最后验证观察器停止异常仍删除精确自有数据库容器。

所有开发输出使用 D/E 盘。数据库实例、数据目录、端口和所有者标记由运行器独立创建；删除前核对完整容器身份、镜像和所有者标记。保留 D 盘数据供审计，不操作共享数据库。

```powershell
$env:PYTHONUTF8='1'
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/formal-undo-fence/live.py
python tools/formal-undo-fence/live.py --fault drop_ok
python tools/formal-undo-fence/live.py --fault block_commit
python tools/formal-undo-fence/live.py --fault delay_ok
python tools/formal-undo-fence-gate/qualify.py tools/formal-undo-fence-gate/qualification-inputs-20261116.json
```

这不是业务提交、完整 Undo 回放、TC 安全验收或完整 WP05。`Fenced` 不直接等于通用 RM 的 `Applied` 或 `NotApplied`。
