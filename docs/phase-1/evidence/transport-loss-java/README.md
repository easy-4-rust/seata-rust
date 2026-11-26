# 正式传输组件的真实 Java TC 响应丢失验证

当前封存运行：[211914](run_20261113_211914_27aea8f7/result.json)，143来源、185成员、一个实际正式驱动binary，catalog SHA256=`76219d90eeb6cdde8a7ab7d67fddf083815545d91af6bfb83f71a5ef8abe8519`。固定原Java pin `cba73e180fd8896a99df6415b859d275122feaa7`及原server JAR SHA256 `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`。三个场景各启动独立TC和只丢一条完整成功响应的代理，真实发送字节不被改写；共3个TC＋3个代理，结束后自有进程和38091/38092监听均清理。

| 场景 | 实际行为及证据边界 |
|---|---|
| NET-011 Begin | 原TC执行成功但响应丢失；正式transport返回Unknown，客户端无XID、不发第二Begin。外部观察器捕获的XID未提供给客户端。保留148字节FILE测试记录证明原请求实际建立了事务；随测试TC退出，不声称客户端完成该孤立事务恢复。 |
| NET-010 Commit | 原XID只注册一次分支，锁先占用；成功Committed响应丢失，收到真实BranchCommit回调并回原连接。客户端先查询原XID，再显式重复一次同XID Commit，没有改成Rollback；模拟动作1次，锁释放。实际查询和重试均返回Finished15，客户端confirmed=false，保留原提交未知，不能从无记录证明历史提交。 |
| NET-012 BranchRegister | 成功BranchId响应丢失；客户端不重试注册，查询原全局状态Begin，锁仍占用。显式取消原XID后，仅从身份/资源/AT/操作nonce完全匹配的真实BranchRollback回调学习BranchId；原连接回应，TC返回Rollbacked11，模拟动作1次，锁释放。观察器的被丢BranchId仅作独立比对，未供客户端恢复使用。 |

每场景冻结7个实际加载类来源（3个core来自原JAR内部core依赖、DefaultCore/AbstractCore/AbstractAuth来自原JAR classes、公共认证fixture来自冻结runtime扩展），以及实际-Dspring.config.location配置、其源文件和运行时SPI两文件。结束时重新核对来源、JAR和client binary；`succeeded`只在业务/清理及输入闭包全部通过后置true。没有编译Java或执行JUnit。

独立观察器重新解码原始帧，核对signed wire ID、原连接、成功前缀、XID/精确i64 BranchId及分支终态；11→12、3/5→4/6和21→22必须按连接/编号对应。回调分支必须来自原成功注册，锁查询必须保留原resource/lockKey和同一独立contender。父级[组合门禁](../transport-recovery/README.md)进一步绑定客户端状态声明、实际投递的16/8状态及操作/回调/锁释放/取消确认顺序。

历史运行：210646因观察器将Java两字节布尔值错误读取为一字节而失败，不计通过；211233三场景功能通过，但后续只读审查发现raw关联/实际配置/成功结果后置不足，保留为回归数据，不作为最终认证。原始证据均未改写。对应实际RED/GREEN及原始帧破坏控制封存在组合门禁中。

这是正式transport＋内存模拟RM的故障验证，不是生产TM/RM、持久幂等、SQL/Undo补偿或完整V1恢复证明。公共认证fixture并非生产凭据策略，Java日志extraData风险仍开放；原Go默认差异、WP00-R、外部TC风险未接受及共享锁延期边界不变。

复现：`python tools/formal-transport-loss/live.py`。仅使用现有D/E来源/缓存及固定公共fixture；新建独立输出，不覆盖旧归档。
