# 受限本地执行及前后镜像门禁

延续既有WP05契约，不初始化新的规格体系。开发输出仅D/E盘；使用已有离线Cargo缓存与固定MySQL镜像，不安装工具，不接入共享数据库。

```powershell
$env:PYTHONUTF8='1'
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/formal-mysql-execution/run.py
python tools/formal-mysql-execution/audit.py --run <上一命令输出的MYSQL_EXECUTION_GATE_RUN父目录>
```

两步都成功才发布。第一步重新执行双Rust工作区及四组真实MySQL场景、隔离实现削弱检查，封存源/产物；第二步只读校验共同输入、唯一声明的变更和各控制的预期失败诊断。原Java schema作为父包之外的一个现场输入，单独绑定此前封存原字节。

各负控使用D盘独立源码与目标目录；预期是编译后目标行为失败，不能以构建失败或任意现场panic充数。正向和负向产物分别归档。精确参数、原SQL一次执行、镜像、内存Undo与取消/超时只在声明的类型/会话/权限范围有效。

[最新证据](../../docs/phase-2/evidence/mysql-execution-gate/README.md)。本门禁没有公开COMMIT，不证明Java Undo日志互通、同事务持久化、正式分支注册或提交未知恢复；WP05/06继续开放。
