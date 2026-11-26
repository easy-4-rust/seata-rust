# WP-02 传输组件与 NET-001–012 组合验收

[正式212118运行](run_20261113_212118_f4704076/result.json)：2634来源、2695归档成员、16个实际执行测试binary，catalog SHA256=`ba1458a8a1e944e4693a552fe6ef201db5b4b2457646170e5aca70637006c5f2`。所有组件/Cargo源、16锁依赖archive校验值及解包源、测试驱动、观察器、既有规格和子归档引用已冻结。没有安装、联网、分支操作或新增依赖。

| 验证 | 结果 |
|---|---|
| Rust1.99及最低1.85 | 各87正式transport测试：24unit＋10pool＋9heartbeat＋7inbound response＋13registration＋10stream/ID＋14原TCP，合计49真实TCP函数、4配置测试。另各3个联调辅助unit，单独计数。 |
| 回包身份与预算 | 消费原代际PoolInbound能力，仅sync请求可回，保持signed ID及nullable有序头；校验完整平面批次和精确branch-end身份。独立有界reply预算，不与同wire ID普通请求future交叉完成。 |
| 回包生命周期 | 完整本地写完且期限内才成功，并非TC/业务ACK；写前过期/取消不写，已写部分帧必须完成或关闭socket。旧代际不迁移，关闭和成功共享同一锁仲裁；无Tokio上下文在入队前返回Local。 |
| 14隔离行为负控 | 6登记负控＋8响应/恢复负控，均以真实目标断言失败exit101通过；独立D副本每项从相同基线重建并最终恢复，不改正式源，也不称14个原产品缺陷。 |
| Python观察器 | 13测试通过，其中13项原始帧/状态声明破坏被拒绝。逐字节核对callback/注册/锁身份与原连接、signed ID、响应状态和先后顺序，不能仅信客户端PASS或事件元数据。 |
| 质量 | workspace fmt、全target Clippy -Dwarnings、rustdoc -Dwarnings通过。 |
| 实际Java登记 | [211350](../registration-java/run_20261113_211350_6a0bee84/result.json)六个现场用例重跑通过；139输入/152成员，catalog=`8ac6fd9f4a2c2e1af44be92ea9f3ed9a1bc35479e73b0fd89815cacb8fe2487f`，5实际类来源/两runtime SPI文件，128共同编译输入与本门禁一致。 |
| 实际Java丢响应 | [211914](../transport-loss-java/README.md)三种独立场景重跑通过；143输入/185成员，catalog=`76219d90eeb6cdde8a7ab7d67fddf083815545d91af6bfb83f71a5ef8abe8519`，每场景7类实际来源/真实运行配置，128共同编译输入一致。父层另从raw重新验证三场景。 |
| 协议绑定 | 既有peer192819的70协议/Cargo编译输入相同；628正向/2050畸形、74＋1双版本、7Python/12输出负控为引用，本轮未重新执行协议peer。 |

API首个RED为缺接口编译错误。后续真实产品RED分别发现关闭先发生仍发布写成功、缺Tokio上下文却先入队写字节；修复后87 GREEN。观察器的u16布尔值及原始帧关联问题均有实际断言RED/GREEN，诊断和来源较弱的早期功能运行不计正式Gate。

NET-010实际Finished15/15保留confirmed=false；NET-011客户端没有观察器XID、不盲目重启Begin；NET-012仅通过真实匹配callback学习丢失的BranchId，再获取原TC取消确认。模拟动作去重不等同持久RM幂等或数据库补偿。运输层不实现自动事务重放，JavaTC仍为事务重试权威。

父脚本没有再次启动Java，result里的`java_tc_started=false`/`external_services_started=0`仅描述父执行。整个里程碑确有新的Java子运行：一个登记TC＋三个故障TC/三个代理，均清理；未执行Java编译/JUnit、MySQL、旧probe117或Go254。

本组合结果满足已批准首期profile的WP02传输组件范围。后续生产TM/RM、持久恢复、MySQL、生产认证/TLS、原Go默认兼容/WP00-R、全部后续阶段与发布仍开放；外部TC风险未接受，共享锁继续延期。规格状态发布在测试封存之后，编译源一致性单独核对。

复现：`python tools/formal-transport-loss/run.py --peer-run docs/phase-1/evidence/protocol-peer-profile/run_20261113_192819_1650e24d --java-run docs/phase-1/evidence/registration-java/run_20261113_211350_6a0bee84 --loss-run docs/phase-1/evidence/transport-loss-java/run_20261113_211914_27aea8f7`。引用子证据前重新校验catalog及原始输入，不覆写既有归档。

[只读终审](readonly-review.json)通过：父/子catalog、16产物、各128共同编译输入、16archive/1850源码、56原始帧、26类来源、三份实际配置及清理全部核对。发布文档更新后编译输入仍一致；不把历史文档快照宣称为当前零漂移。[WP02状态](../../wp02-transport-gate.json)已记录12/12及后续范围。
