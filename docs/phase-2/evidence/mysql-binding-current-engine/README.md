# WP05 资源绑定当前物理引擎

沿用 WP05 与 tc-lock-coverage-contract.md 的既有物理绑定要求，不创建新的规格体系。

真实 RED：业务读取形成字典快照后，管理员把绑定表改为 MyISAM。旧准备通过并实际发送原格式分支登记，收到真实受控协议回执；准备作用域仍持有时，管理员改写绑定行成功 1 行，证明原共享行锁要求失效。独立 SQL 观察业务/Undo 各 0、恢复登记 1、绑定值 00、实际引擎 MyISAM。red.log、red-observation.json、red-database.log 共同记录事实；该受控对端不是原 Java TC。

修复在原 FOR SHARE 的 MDL 下调用已有物理建表定义引擎检查，不再只依赖旧 information_schema 快照。在原连接和期限内拒绝迟到非 InnoDB 或视图，发送 BranchRegister 之前关闭原业务作用域。无新增连接或认证/TLS 变化，不扩大为完整绑定表 schema 认证。

新增现场案例覆盖 MyISAM 与迟到视图替换，要求精确当前引擎 Unsupported、零分支请求、无回执且非登记未知，并独立核对业务/Undo/恢复记录零。原准备测试继续覆盖正确绑定的共享行锁、MDL 持有、原字节、任一端要求绑定的策略，以及超时/取消。提交前置组覆盖原恢复/Undo仲裁回归。verify.py 核对每个 Rust 版本的 12 项准备、13 项提交前置、635 项工作区测试、严格 Clippy 和清理后生成 ../../wp05-binding-current-engine.json；普通测试忽略 197 项环境案例。

首次链接受磁盘压力影响，进程长时间无输出、I/O/CPU 不变。只终止本轮明确匹配的链接进程，保留诊断及 LNK1318 日志，清理匹配路径的过期 PDB 后删除这个中断目标的 PDB 重建。未修改测试断言、程序或数据库文件。清理清单和 interrupted-*.json、1.99-link-disk-pressure.log 保留。

独立只读审查未发现阻断项。完整原 TC 锁安全、公开业务 COMMIT、WP05/WP06 及全部后续计划门槛保持开放；稳定单主路由、各客户端统一原 JDBC 资源 ID 和显式停写迁移要求保持。
