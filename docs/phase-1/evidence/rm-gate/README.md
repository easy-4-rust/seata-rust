# WP04 正式 Rust RM 首期验收

本页对应原 [WP04 十项与 Java Gate](../../../phase-0/v1-implementation-acceptance-checklist.md)。[合并运行020104](run_20261114_020104_c0699263/result.json)已完成执行并通过[只读终审](readonly-review.json)。范围为获批的原生 V1 RM 控制面、文件恢复契约及真实 Java TC 互操作；MySQL 业务 SQL、Undo Log 和数据库原子性属于后续 WP05/06。

合并记录2076输入、2113成员，catalog SHA256=`3583c327f3fd939a4a5ad007755ac9fa8d317c1cfcd844fe72f8d0612e733ef4`。它逐项核验两个不可变子包的全部成员与快照，并重新解析原始双端帧、原 TC 日志及分支日志，不仅相信客户端 PASS 或子包的 verified.json。

| 验证层 | 实际执行与证据 |
|---|---|
| 正式组件 | 继承[运行组件003214](../rm-runtime/run_20261114_003214_ee6afbb3/result.json)：Rust1.99/最低1.85各234正式测试，其中45RM；52测试产物、11实现削弱断言、12真实恢复子进程。合并检查2044项当前编译/依赖输入一致；本次没有重跑234项。 |
| 真实 Java TC | 新执行[014916六例](../rm-java/run_20261114_014916_69ccac54/result.json)：正常、重复、断连各提交与回滚；6个 TC 与12个观察代理真实启动，54个关键类实际加载来源。子包2075输入/2169成员，catalog=`c4763b18ce8143b932683d4a4c95a91fb57b6e7dfd3fabd0cd26147d3d574da2`。 |
| 独立观察 | RM 与 TM 分别保存原始 V1 帧；原 TC stdout 独立绑定两次具名 Begin、实际签发的 XID 及唯一原操作。两代理使用同一主机高分辨率计数器，绑定锁冲突、全局结束、RM 回调、实际交付回包和后续状态查询的顺序。 |
| 校验程序 | 38项 Python 单测；10份 D 盘隔离削弱的校验器全部通过编译/导入，并在指定测试出现真实 `AssertionError: ValueError not raised`。不把缺模块、语法错误或空运行计作负控。 |
| 联调辅助资源 | 独立工具 workspace 两项测试在Rust1.99/1.85各通过，保存两个实际执行产物。资源夹具重复调用直接失败且计数递增，不用夹具自身幂等掩盖 RM 重复执行。正式 RM 不依赖 TM。 |
| 来源与清理 | 原 Java pin/JAR、公开测试 SPI、实际配置、22个锁定依赖归档和1835个当前平台依赖源绑定；自有服务与38091/38092/38093监听清理检查通过。没有新 Java 编译或 JUnit 运行。 |

| Java 场景 | 原 TC 回调数 | 实际 RM 回包数 | RM 登记次数 | 资源 apply/effect | 首次全局结束观察 |
|---|---:|---:|---:|---|---|
| basic-commit | 1 | 1 | 1 | 1/1 | Committed=9 |
| basic-rollback | 1 | 1 | 1 | 1/1 | Rollbacked=11 |
| duplicate-commit | 1，原字节实际投递两次 | 2 | 1 | 1/1 | Committed=9 |
| duplicate-rollback | 1，原字节实际投递两次 | 2 | 1 | 1/1 | Rollbacked=11 |
| reconnect-commit | 2，TC真实重试 | 2，首次终态回包被丢弃并断开 | 2 | 1/1 | Committed=9 |
| reconnect-rollback | 2，TC真实重试 | 2，首次终态回包被丢弃并断开 | 3 | 1/1 | 客户端原请求结果未知；稍后原响应Failed=0、exception=0、RollbackRetrying=5 |

全部场景精确保留原 TC 签发的正 i64 BranchId（实际超过2^53）、XID、资源/类型与应用 nonce。分支只注册、报告一次；结束前两个锁查询分别观察拥有者可用、竞争者冲突，完成后第三次查询观察竞争者可用。每个文件日志仅有原身份的 Pending→Completed 两条记录，校验 CRC/序号/边界，并实际关闭重开。重复模式扣留第一回包，直到第二份相同回包到达才交付原回包；断连模式仅丢原回包、关闭原连接，后续请求由 TC 自己发起，代理没有伪造回调。

`reconnect-rollback`保留原“未知”诊断，不拿15秒预算的TC原响应迟到Failed重写5秒预算下客户端报告的未知。校验器只允许原已未知的目标结束请求出现该迟到失败，其他 TM 调用要求明确成功；竞争事务回滚必须实际返回11。所有 `OBSERVE` 逐项匹配相应状态响应，Finished=15仅证明清理观察，不证明历史提交/回滚。客户端超时发生的精确时刻没有独立追踪，未声称已证明该时序；原请求不被自动重发这一点有原始帧与 TC 日志证据。

早期010223/010714为较弱单端观察记录，012911因低分辨率时钟不能证明严格顺序而拒绝；013524前五例通过，最后一例发现校验器错误拒绝合法迟到Failed，均不能替代最终014916。父门禁首次015742仅因两种历史catalog容器格式读取错误退出；015811控制实际执行源与归档仅换行格式不同，经审查后改为归档实际D文件原字节，重跑最终020104。两旧父记录均不计产品缺陷或行为RED，且不能替代最终父包。真实时序、编号复用、恢复计数、时钟和竞争者状态的校验缺口修复及负控分别有记录；不声称正式 RM 在这一步新增过网络行为缺陷。

原登记期间缓存回调、完整当前资源修订Ready后转发的规则保持不变。资源 `NotApplied` 必须围栏所有旧尝试，具体适配器仍须自行证明；文件资源测试不能证明数据库迟到提交围栏、SQL/Undo原子性或独立sync故障。生产认证/TLS、WP00-R、原 Go 默认兼容、完整 V1 与 Phase4–7继续开放；外部TC风险未接受，共享锁按用户要求暂缓。

重跑真实互操作：`python tools/formal-rm-java/live.py --cases basic-commit,basic-rollback,duplicate-commit,duplicate-rollback,reconnect-commit,reconnect-rollback`。`tools/formal-rm-java-gate/run.py`为本次固定证据的合并核验与负控入口；依赖或正式源码改变后必须重新取得受影响的组件与互操作证据，不能沿用历史结果。
