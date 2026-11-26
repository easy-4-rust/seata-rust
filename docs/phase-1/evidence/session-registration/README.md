# WP-02原生会话登记与版本化资源就绪（2026-11-13）

正式运行：[run_20261113_202518_b319cc2a](run_20261113_202518_b319cc2a/result.json)，2590来源/2628归档成员/12实际测试binary，catalog SHA256=`38f8255d69b57f15eaeb43ea05a3b7927c079d76014296e8eff9eae75437f107`。16锁定registry依赖archive checksum与解包源码、所有正式组件源/Cargo输入、规格及子证据引用封存，无新依赖。

| 验证 | 实际结果 |
|---|---|
| 两工具链 | Rust1.99/最低1.85各76：20unit＋10pool＋9heartbeat＋13registration＋10stream/ID＋14原TCP；总42真实TCP函数/4配置测试 |
| 身份/失败分类 | TM未确认不发业务、明确拒绝不ready、首个RM资源兼顾资源型auth策略、错误响应族为Unknown、登记响应丢失保留Unknown、本地帧预算失败保持Local，后续项NotAttempted |
| 动态/代际 | 新代际全量登记所有所需资源；新增资源立即使旧revision失效；初始化中再次变更取消旧future，迟到旧reply只观测，当前完整计划通过才Ready |
| 生命周期/预算 | 单一100ms初始化期限覆盖多步骤，不能逐资源重启；注册中shutdown/Drop取消等待并关闭socket；完整20个原有等待/计时/ID等unit亦回归 |
| 六项隔离负控 | identified=false误接受、Registering可选、旧revision保留Ready快照、静默截断所需资源、逐资源重算期限、额外RM空资源握手，均触发实际目标断言失败exit101；同一D副本逐项从基线重建并恢复 |
| 质量 | workspace fmt、全target Clippy -Dwarnings、rustdoc -Dwarnings通过 |
| 实际JavaTC | [增强来源现场202129](../registration-java/README.md)六用例通过，5实际Java类来源、运行时fixture两文件、JAR/SPI哈希/自有PID与端口清理、FILE全局存储0；122实际Rust/Cargo输入与父一致 |
| 协议绑定 | 既有peer192819的70编译协议/Cargo输入一致，628/2050、74＋1双版本/7Python/12负控为引用；本轮没有新执行协议peer，不把旧结果充新测试 |

API初始RED仅缺接口编译错误，不是七个运行断言失败。兼容问题有实际RED：RM首个空资源身份请求不满足合法资源白名单auth策略；现非空缓存以首资源请求同时建立身份，identity/resources[0]引用同一次attempt。空缓存才发RM(None)，TM仍101，V1 identified是唯一确认，不伪造普通ResultCode。六隔离变异为刻意回归控制，不称六个原产品缺陷。

[只读终审记录](readonly-review.json)逐SHA核对父2628成员/2590来源、12产物、16依赖archive及1850源码文件、六负控与恢复基线；子Java146成员/133输入/实际client binary、122共同编译输入和五实际类来源/两runtime fixture文件亦核对。现场六PASS与程序断言一致，store0、38091无监听，未见本轮自有进程残留；复审未重新运行测试/服务。后续README/清单更新与编译源一致性分开记录。

现场JavaTC在先前独立子checkpoint新启动并成功清理，父门禁器只校验/引用该子归档，没有再次启动TC。因此父result中的java_tc_started/new_java_execution=false及external_services_started=0仅描述该归档脚本；整个里程碑存在新Java现场执行，详见live-java-reference的显式标记。不曾重新编译Java或执行JUnit。

NET008/009的注册组件场景已获正式代码和现场证据，WP02为11/12，完整Gate仍OPEN。NET010–012事务协调、TM/RM业务回包、MySQL Undo/补偿、生产认证/TLS和全部后续阶段仍必需；资源删除/持久分支去重亦未完成。既有Go默认兼容/WP00-R、外部TC风险未接受及共享锁延期不被此门禁改变。

复现：`python tools/formal-registration/run.py --peer-run docs/phase-1/evidence/protocol-peer-profile/run_20261113_192819_1650e24d --java-run docs/phase-1/evidence/registration-java/run_20261113_202129_a18c4375`。需要固定D/E来源和缓存，新建证据/隔离副本，不覆写旧归档；执行时快照与随后README/验收状态更新分开，编译源另外核对。
