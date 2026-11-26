# WP03正式TM完整组件门禁

[最终224638](run_20261113_224638_2590f234/result.json)：2090输入、2175成员、24个实际执行测试binary，catalog SHA256=`f59fdd96a66d2a1b8768a9c0317bbe1464fe97e7797847ba66798bb11f92e4d2`。[WP03机器状态](../../wp03-tm-gate.json)现验证通过，下一步WP04正式RM控制面；这不是完整V1或生产恢复完成。

| 验证 | 范围与结果 |
|---|---|
| 双Rust版本 | Rust1.99/最低1.85各28正式TM＋87传输回归；另各2个联调辅助unit，单独计数。24 binary均实际执行，包括两个0测试TM lib binary，不将其计入测试数。 |
| 19隔离负控 | 10个正式TM行为变异＋9个独立验收器变异均实际目标断言失败。D副本每项从同基线重建，结束恢复；不改正式源，不称19个原产品缺陷。 |
| Python观察器 | 13方法通过，含五种正向状态组合和原始帧/声明反例；旧proxy另外7方法通过。唯一原始应答、丢响应后发起查询、同意图、确切分支/nonce/连接、锁资源/key、原TC成功状态及客户端未知标记均受反例保护。 |
| 基础Java | 引用[221553六功能场景](../tm-java/README.md)：七个空全局事务、原TC日志7Begin/4Commit/3Rollback、6实际类来源、配置/SPI/产物和清理；155共同Rust/Cargo输入一致，未在本轮重跑该子场景。 |
| 真实TM丢响应 | 新[224303三场景](../tm-loss-java/README.md)：Begin不启动业务或重发；Commit/Rollback同XID同操作先查询再显式重复，实际15/15均保留Unknown，Rollback保留业务错误；每分支内存动作1次、锁释放。156共同输入一致，增加的一个文件是新driver；生产库未改。 |
| 原始观测/来源 | 父层重新核验三故障58帧，每场景唯一完整成功响应被丢弃，恢复15请求在drop之后且16在retry之前。基本6＋故障3×7共27类加载来源、运行配置、SPI、两个client产物均匹配；自有TC/代理及38091/38092均清理。 |
| 协议与依赖 | 现有peer215647的71相关编译输入一致；74＋1双版本、628/2050、7Python/12输出负控为该既有归档引用，本轮未重跑peer。16crate archive及1850源码文件逐SHA核对，无新依赖、安装或联网。 |
| 质量 | workspace fmt、全target Clippy -Dwarnings、rustdoc -Dwarnings通过；[只读终审](readonly-review.json)无新阻断。 |

新driver通过正式`execute`生成原结束错误，使用本管理器私有RecoveryToken恢复。它不把外部观察器捕获的身份提供给客户端，也不自动重试事务或把未知Commit改为Rollback。若获得明确匹配终态，可以不重复；Finished单独不使未知消失。正向观察器控制覆盖两种明确终态路径，现场两个结束模式实际均15/15。

验收器复审两P2有实际RED四处断言失败：重复投递原响应、旧在途查询冒充恢复。修复后13方法GREEN及九个独立削弱变异全部被抓住；最终现场使用修后验证器新跑，未用历史弱验证器的succeeded替代。最初missing-validator API RED只是模块缺失错误，不计运行断言失败。

父脚本自身没有启动Java，顶层result的`java_tc_started=false`与external0只指父归档器；新故障子运行确有3TC＋3代理。六基础功能使用较早已冻结子checkpoint。本轮没有Java编译/JUnit、MySQL、旧probe117/Go254，未把历史协议peer或基础Java结果称为新执行。

已批准首期profile的WP03 Gate完成；RM分发/持久幂等、SQL/Undo补偿、生产认证/TLS、跨进程恢复、原Go默认兼容/WP00-R、其余Phase、完整V1和发布仍开放。共享锁继续延期，外部TC风险未接受。源码、运行结果和发布后的清单/README分开：当前编译输入不变，不宣称所有历史进度文档零漂移。

复现执行时快照：`python tools/formal-tm-loss/run.py --java-run docs/phase-1/evidence/tm-java/run_20261113_221553_243ee496 --peer-run docs/phase-1/evidence/protocol-peer-profile/run_20261113_215647_eee62e75 --loss-run docs/phase-1/evidence/tm-loss-java/run_20261113_224303_f22324e1`。脚本严格复核导入来源；若发布文档已与该epoch快照不同，需重新运行子runner并替换新run参数，不能将旧来源改写来伪造当前闭包。
