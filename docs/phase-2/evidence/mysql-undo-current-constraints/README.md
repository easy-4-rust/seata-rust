本目录验证独立 Undo 补偿原事务的字典快照早于业务表约束 DDL 时的拒绝行为。

RED 使用真实 MySQL 的 LOCK TABLES 阻塞业务表 MDL，并在 performance_schema 观察到 PENDING 后提交触发器，再释放锁。旧实现返回 Consumed，独立查询得到业务行 0、Undo 行 0、意外触发器副作用行 1。对应 red.log、red-observation.json 和 red-database.log。

GREEN 在任何补偿 DML 前，使用原数据源连接配置的新会话核对当前触发器、入向及出向外键。新会话核对原实例 UUID 和 schema，原事务持续持有业务表 MDL。两个新增测试覆盖三类迟到约束，拒绝时核对提交前分类、原 XID/分支、原 Undo 七列以及业务/子表/副作用行状态。完整独立 Undo 和托管 RM 现场回归分别为 16 项和 11 项。

托管入口使用同一生产检查，接线和期限由独立代码审查核对。迟到约束的精确锁调度通过独立回滚入口执行，本目录不将其描述成真实 Java TC 回调验收。

运行记录保留了夹具设置错误。missing-child-env 是首次未提供跨进程恢复子程序，existing-fixture-user 是重跑保留账号，reused-journal 是重跑复用了已完成文件日志。最终运行先编译精确工具链子程序，重置自有实例的指定账号，每个测试套件使用新建的 D 盘输出目录。最终通过结果以版本名开头的 live_undo_rollback.log 和 live_rm_recovery.log 为准。所有这些操作均只针对本轮自有 MySQL 实例。

run-live.ps1 记录最终补充回归命令，run-workspace.ps1 记录双工具链工作区及 Clippy 命令。原 MySQL DDL 与固定 Java 源文件按文本比较，允许文件换行差异。验证器检查日志和摘要，不代替数据库测试。

辅助检查每次临时增加一个池外连接，稳定单主路由要求继续适用。公开一阶段提交及原计划其他门槛不由此记录宣布完成。
