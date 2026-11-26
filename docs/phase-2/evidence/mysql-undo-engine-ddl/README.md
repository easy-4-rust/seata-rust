本目录验证业务事务已建立字典快照后，Undo 表实际引擎发生变化的原子性缺口。

真实 RED 先由正式本地事务执行业务 INSERT，再由独立管理连接将 undo_log 改为 MyISAM。旧校验仍接受旧 InnoDB 字典字段，flush 返回成功。原事务 Drop 回滚后，独立 MySQL 查询得到业务行 0、Undo 行 1，当前引擎 MyISAM。对应 red.log 与 red-database.log。

修复在既有 validate_table 完成七列及唯一身份校验后，使用同一个原事务读取当前 SHOW CREATE TABLE，复用已有业务表的规范引擎提取规则。初始空锁定读取已持有 Undo MDL，所以此检查与后续写入之间没有 DDL 窗口。原绝对期限、失败污染及取消丢弃策略保持，无公开 API 或额外写入连接。

GREEN 精确要求 current physical engine must be InnoDB，不能由旧字典的早期拒绝代替。用例覆盖普通 MyISAM，以及列注释、表注释中的换行与带尾空格的完整 InnoDB 伪词元。各例在第一条 Undo 写入前拒绝，原事务保持污染，回滚后业务和 Undo 均为零。恢复 InnoDB 后同一个数据源的新事务可以 flush 并整体回滚，两表仍为零。

三份 ddl-current-definition JSON 保存与原会话相同 NO_BACKSLASH_ESCAPES 设置下的原始 SHOW CREATE 字节。MySQL 8.4.13 将注释换行显示为字面反斜杠+n，每份只有真正的 MyISAM 引擎行。此结论限定固定服务器配置，不声称任意版本、任意 DDL 输出解析均正确。业务表既有引擎 DDL 用例也重新通过。

中间误让两个共享数据库夹具的现场进程重叠，导致相互删表。两份 fixture-overlap 日志保留但不计作生产缺陷或成功证据。确认进程结束后加强用例单独通过，再由唯一运行器按顺序重跑完整 11 项写入和 16 项补偿。断言补丁曾匹配到后面的外键测试，交付前已纠正并再次完整运行写入套件；最终外键测试保留原通用拒绝断言，新用例精确匹配物理引擎错误。

最终结果以版本名开头的 live_undo_store、live_undo_rollback 和 business-engine 日志为准。两个工具链各另通过 635 项工作区测试和严格 Clippy，普通运行忽略 193 项环境测试。只有原始 DDL、日志与摘要归档，不复制全部构建缓存。自有 MySQL 使用固定镜像和 64 MiB redo 容量，减少夹具磁盘占用。

本修复只关闭当前 Undo 物理引擎检查缺口。其他 Undo/恢复表的列、索引、约束快照、TC 安全及公开一阶段提交仍按原 WP05 计划继续。
