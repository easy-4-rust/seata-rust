# WP05 MySQL物理列与UPDATE/DELETE前镜像

[最终执行](run_20261114_045819_c62f4e63/result.json)、[只读终核](readonly-review.json)、[发布复核](publication-audit-20261114.json)及[部分状态](../../wp05-mysql-before-image.json)落实[原实施契约](../../mysql-row-read-contract.md)。原WP05十四项/Gate继续开放，当前不包含INSERT镜像、业务SQL、after image、Undo或提交未知恢复。

最终冻结7952输入、10224成员，catalog SHA256=`10285b1534c237d5f1eab8944aa039851a70a9798eedd0f80a66bccdb1993ea6`。Rust1.99与最低1.85各实际执行282工作区测试，含新增8项物理域与1项请求预算测试；保存72个实际工作区产物。每版本另在两个独立自有MySQL容器执行metadata9项和row10项，共19项实际数据库正向场景，并保存四个独立现场产物。另八项普通实现削弱及五项真实MySQL实现削弱均成功编译、在指定目标断言失败；五个现场负控产物独立归档，不能与正向通过混计。既有RM文件恢复四组12子进程再次执行；不是SQL/Undo进程恢复证明。

| Rust | 场景集 | 实际子记录 | 正向通过 | catalog SHA256 |
|---|---|---|---:|---|
| 1.99 | metadata | [run_20261114_045946_8cef695b](../mysql-metadata/run_20261114_045946_8cef695b/result.json) | 9 | `771302a8c3ff427c031d8ddb202cde3c015be7cefa8b0e787b5c4ae9a95ecc74` |
| 1.99 | row_images | [run_20261114_050023_f0107558](../mysql-row-images/run_20261114_050023_f0107558/result.json) | 10 | `0f7cdd6a2b448756ba9408479dbaa84006c5f853d837d02036162f0827f5a6c1` |
| 1.85 | metadata | [run_20261114_050056_0e148194](../mysql-metadata/run_20261114_050056_0e148194/result.json) | 9 | `06f6fc25a56c544e2e4b00bce7bcb2d450464490013d1ba620ceae34ac8a231d` |
| 1.85 | row_images | [run_20261114_050132_cc694f62](../mysql-row-images/run_20261114_050132_cc694f62/result.json) | 10 | `4fe1a5c8f3cba75d2e934fc5e2bde709bcfa1e1ca16d4c51978edd5e878d9fe5` |

读链在同一本地事务内取得MDL、核对物理域、按真实复合PK索引排序，只读取长度/NULL标记并持行锁；确认1024行/单字段1MiB/总8MiB payload预算后，才读取完整列。实际查询原WHERE参数切片，精确i64/u64/BigDecimal原生绑定、65位DECIMAL/30位scale、UTF-8/ASCII/UTF8MB3、NULL/空值及任意二进制均已核对。原业务UPDATE/DELETE不在此执行。

物理域覆盖整数8/16/24/32/64位与unsigned、CHAR/VARCHAR字符数/字符集、TEXT字节、BINARY补零长度及UNSIGNED DECIMAL。旧八参数ColumnMeta构造保留None域及旧内存行为，真实适配器填充域，真实镜像读取拒绝缺失域。范围仍为当前已声明类型，不是全部JDBC类型或全部MySQL配置。

现场general_log独立确认三种预算拒绝前没有执行完整payload SELECT；负控取消长度预检后会读全值，并被该断言发现。SET/WHERE错片、将DECIMAL伪装文本、把Future事务所有权留在外层、按物理列序排序PK，也分别触发真实目标失败。取消测试先确认原事务MDL令独立ALTER返回1205，再扣响应/取消，且在代理关闭前独立ALTER成功；单凭43字节不被当作已持锁证明。原目标输入和回包未伪造。

现有并发场景证明返回镜像后更新被锁阻塞直到回滚；尚未独立调度两次查询之间的并发插入/字段膨胀。当前RR与同事务锁是设计前提，后续补该精确竞争场景，不能用已有场景替代。客户端期限针对整读链，失败/取消保持poison；没有严格实时调度精度声明，也不声称防御恶意MySQL服务端返回超大网络包。

服务器均为8.4.13、lower_case_table_names2，含大写及美元符号FK表拒绝。四个正向加五个负控共九个自有容器均由owner标签和固定image核对后清理，原数据目录保留D盘。该数字是容器/受测服务实例数，不是官方入口所有mysqld启动次数。测试显式loopback非TLS，权限仍需TRIGGER及全局PROCESS；生产TLS/最小权限未完成。

锁文件192包，其中185registry原包按checksum核验；实际cargo tree为148registry，与两个版本实际compiler-artifact集合逐项相等，其6745源码逐tar成员匹配。metadata候选图159包，不能当成真实编译闭包；其中home1.88属于未编译的可选Postgres路径，未宣称其可在Rust1.85编译。只读公开C缓存后复制D盘，没有网络下载/工具安装。[旧计数澄清](../mysql-foundation/dependency-scope-audit-20261114.json)保留历史封存包，说明旧151是候选、实际编译146。

[绑定审计](core-binding-audit-20261114.json)确认协议、传输、TM、RM和AT原源码不变，旧178锁包版本/checksum保留，仅新增14锁包与SQLx原生DECIMAL feature。旧Java/Go Gate仍属历史证据，本轮未新执行。九Java对象PARTIAL、两个beforeImage方法原源引用、189测试文件仅库存、七资产原字节复制；完整Java案例/序列化互通仍未完成。

初043521因初始化CLI缺SET NAMES导致emoji分裂，实际0测试；修复fixture后043633真实暴露旧元数据域None，属于目标assert RED。缺API为编译RED，早期044354九例取消证明偏弱，045644十例补强后再通过；最终父再次完整执行，并保存前述原失败/源码。WP05/06、生产认证、WP00-R、原Go默认、完整V1、原生RustTC及所有后续Phase继续；外部TC风险未接受，共享锁暂缓。

复现 `python tools/formal-mysql-row/run.py`，开发/运行输出只在D/E盘。
