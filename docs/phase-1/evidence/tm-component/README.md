# WP03正式TM组件实施与基础Java验证

[221732正式归档](run_20261113_221732_9b4ea354/result.json)：2058输入、2122成员、22个实际执行测试binary，catalog SHA256=`a41801b7fcad2ba66f28dd99046c5f62a5f45ad7248621e9c4fa4a5dfaeaeed8`。本归档验证TM实施组件，完整WP03 Gate仍开放；下一步补正式TM的真实Java响应丢失。

| 验证 | 实际证据 |
|---|---|
| Rust1.99/最低1.85 | 各28TM测试（7配置/上下文、13调用/恢复边界、8作用域执行）＋87既有transport回归。11个target各版本均实际执行，包含0测试的TM lib binary；不把它计入28。调用/作用域测试使用真实TCP故障peer；伪造诊断拒绝测试本身只验证本地授权边界。 |
| 公共API/状态 | 原生TM登记池、精确XID/可选Begin附加数据、完整Failed前缀及报告字段、异步/Finished/重试超时状态、不可连接的写前错误、写后协议/超时未知等均保留原语义。 |
| 上下文/传播 | 六策略、嵌套原角色/期限、同任务交错poll、显式spawn传播降为Participant、panic/取消恢复；业务错误与回滚失败均返回，业务期限不能产生Commit成功。 |
| 未知结果 | 不盲目再Begin，UnknownCommit不转Rollback；同操作重复先查询；Failed/Finished不抹除原未知；查询前撤销旧许可，失败查询不能沿用，16/17/stop状态保留给运维。 |
| 恢复来源 | 私有RecoveryToken绑定本管理器、XID与原操作；改诊断字段、伪造错误、跨管理器恢复拒绝。有效原错误可以恢复受限句柄，仍须查询，不生成新XID或TC期限。 |
| 十项隔离负控 | 写后未知误判Local、Participant发送全局二阶段、失败查询沿用旧许可、参与传播保留Launcher、Finished抹除未知、FailedBegin丢报告字段、Builder Debug泄露名称、恢复跳过查询、忽略签发管理器、有效业务期限反转，均触发实际目标断言失败exit101；不是十个原产品缺陷。 |
| 质量/来源 | workspace fmt/全target Clippy/rustdoc -Dwarnings通过。16锁定crate archive checksum及1850依赖源文件封存；不安装、不联网、不新增依赖或切分支。 |
| 真实Java | [221553](../tm-java/README.md)六场景/七个空全局事务，原TC日志7Begin/4Commit/3Rollback、6实际类来源/配置/SPI/清理核对；父与子155份Rust/Cargo输入一致。 |
| 新协议回归 | peer215647：1849来源/1916成员，catalog=`23b5d8072aee9037757604763d5c073164dc564394f97f9702e3418a2c817919`。新跑74协议＋1验证器双版本、628正向/2050畸形、7Python/12实际输出破坏控制；71相关编译输入与新工作区一致。原Go默认路径仍不因此变兼容。 |

最初三个API RED是缺接口编译错误，不计运行断言失败。后续实际断言发现并修复：事务Debug展示附加数据、Failed/Finished擦除原未知、失败查询沿用旧重试许可、Builder Debug展示事务名，以及公开错误字段可伪造/改变恢复意图。恢复来源两份RED分别证明真实Commit改成Rollback及伪造协议错误可被旧入口接受；现在私有凭证阻断，实际28 GREEN及隔离负控验证。早期220916归档虽然测试通过，但其恢复来源P2尚未关闭，不作为最终组件认证。

父脚本未再次启动TC，子checkpoint确实新启动原TC并清理，引用与新执行分开记录；未执行Java编译/JUnit、MySQL、旧probe117/Go254或再次重放此前全部transport负控。协议三方回归是新执行，使用既有实际Java字节来源与显式Go0001–0006真实API；本轮未新跑Java codec JUnit。

[只读终审](readonly-review.json)核对父2122/2058、Java179/167、peer1916/1849全catalog/快照；22产物、16archive/1850依赖源、155/71共同编译输入、10负控patch与恢复副本、真实日志/6类来源/配置/SPI及自有进程端口均通过。发布README/清单/approval在执行后，编译输入单独核对。

实施清单12项的组件行为已验证；[WP03状态](../../wp03-tm-gate.json)完整Gate仍OPEN，正式TM真实Java响应丢失尚未完成。空全局事务不证明RM/SQL/Undo、持久幂等、跨进程TM恢复或生产认证/TLS。全部后续计划、原Go默认兼容/WP00-R、完整V1/发布仍开放；共享锁继续延期，外部TC风险未接受。

复现：`python tools/formal-tm/run.py --java-run docs/phase-1/evidence/tm-java/run_20261113_221553_243ee496 --peer-run docs/phase-1/evidence/protocol-peer-profile/run_20261113_215647_eee62e75`。

后续[224638组合门禁](../tm-gate/README.md)已用新的正式TM真实Begin/Commit/Rollback丢响应证据关闭WP03 Gate；本页原组件/基础Java的开放状态保留其历史范围，未将旧运行改写为包含故障注入。
