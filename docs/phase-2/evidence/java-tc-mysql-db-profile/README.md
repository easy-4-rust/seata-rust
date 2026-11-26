# 原版 Java TC MySQL DB 锁存储组合配置

`MySqlOptions::with_java_tc_mysql_db_lock_profile()` 同时启用已有资源绑定、锁表字段长度和文本主键排序规则校验。默认及单项配置行为不变，原 TLS 选项保留。

`red.log` 保存缺少该 API 时的 E0599 编译失败。实现后 Rust 1.99/1.85 各通过 2 项定向单元测试、严格 workspace Clippy；`fmt.log` 为空表示格式检查通过。首次严格 Clippy 因新测试使用 unwrap 而失败，原始日志保留于 `1.99-clippy-before-test-lint-fix.log`；测试模块按现有夹具惯例局部允许 unwrap，生产代码不使用。

既有真实 MySQL 准备用例已改用组合入口。两工具链各执行一次完整 `registration_guard_cases`：三次无分支请求的本地拒绝、一次成功登记；成功准备期间绑定行共享锁和表 MDL 均持续至原事务回滚，业务与 Undo 计数为零。原始测试日志 `1.99-live.log`、`1.85-live.log` 与独立观察 `1.99-binding.json`、`1.85-binding.json` 留存。测试使用受控协议对端，不证明原 Java TC 的完整锁覆盖。

本轮自有 MySQL 8.4.13 固定镜像、中继与隔离引擎已清理；`image.log`、`independent-server.log`、`cleanup.log` 保留身份和清理结果。Windows Python alias 启动了真正监听的子进程，首次清理按父 PID 检查失败；实际监听 PID 与父子命令核实后仅停止该两进程，原拒绝保留在工具输出，清理脚本记录最终身份检查。

此配置要求部署方应用 005 迁移、核对固定原 TC 的实际 DB 锁表定义和排序规则、稳定单主路由、全部相关写入器统一原 JDBC 资源 ID。该 API 不探测 TC，也不能证明外部客户端的锁覆盖。公开一阶段 COMMIT 与原 WP05/WP06 门槛仍开放。

原 Java TC 联试夹具已增加 `SEATA_TC_DB_LOCK_PROFILE` 模式；`live_guard.py --full-db-profile` 传递该模式并分配单独证据目录。两工具链的 `live_commit_direction_java` 目标已实际编译，严格 Clippy 复验通过，`runner-help.log` 显示新选项。

固定原 Java TC 与组合配置已在 D 盘自有 MySQL 上执行，最终证据位于 `java-tc-final-1.99/`、`java-tc-final-1.85/`。每套六组物理资源/复合键身份案例，原 RM/TM 帧分别观察 3 次登记（2 成功、1 拒绝）、2 次锁查询、6 次全局回滚和 3 次本地登记前拒绝；1.99/1.85 原 TC 回调分别为 3/4 次，其中重试为 1/2 次。独立最终 SQL 均显示全局事务、分支、锁、业务行零。按源码固定的 Java pin/JAR/Connector/J 校验、编译目标哈希和原报文均在各次输入/观察文件中绑定。`verify_final.py` 重新检查关键结果及清理，并生成 `../../wp05-java-tc-mysql-db-profile.json`。

第一次组合运行沿用历史 `as_cs` 文本主键夹具，被组合排序规则校验安全拒绝；原失败与部分报文保留于 `java-tc-verified-1.99/`。只在组合模式下把正向夹具改为与原 TC 锁表相同的 `utf8mb4_0900_ai_ci`，历史诊断模式仍为 `as_cs`。修改测试参数传递后两套工具链再次按当前源码完整运行，之前的 `java-tc-combined-1.99/` 作为中间证据保留。自有 Java TC、观察进程、双中继和隔离 MySQL 引擎已清理。

这组结果是固定原 TC、选定资源别名和复合键案例的现场验证；其他客户端、所有排序规则和完整排他覆盖仍需原 WP05 门槛，公开业务 COMMIT 继续关闭。
