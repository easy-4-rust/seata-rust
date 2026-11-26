# WP04 正式RM运行组件验证（Java RM Gate尚未完成）

[最终记录](run_20261114_003214_ee6afbb3/result.json)：2615来源、2756成员、52实际测试产物；catalog SHA256 `66e1711307ef6a78c94ed1bde0acdd9e1ce1492bb035d6fbf133d9599490cad4`。22依赖归档及18个当前Windows MSVC平台依赖的1835实际解包源文件逐项比对并前后冻结；4个其他平台归档不代表其他平台已编译执行。

正式 `ResourceManager` 拥有已登记RM池、有限回调JoinSet及关闭顺序，`ResourceClient` 单次发送BranchRegister/Report/LockQuery。错误保留完整原请求/Failed响应，区分未发送、明确拒绝、未知及协议失败；不补造BranchId、不自动重发。回调按精确类型/资源分派，沿原PoolInbound能力回包；整批预校验和原绝对期限覆盖资源动作，结束/取消留下持久可恢复状态。

| 验证 | 实际结果 |
|---|---|
| 双版本回归 | Rust1.99/最低1.85各234正式测试：74协议＋87传输＋28TM＋45RM |
| RM45 | 3unit、12journal、8dispatch、2真实进程恢复、5native_calls、8native_callbacks、7native_edge_cases |
| 所有者与失败 | 关闭/Drop让全部克隆失效；资源Future清理panic必须返回错误并保留Pending；有限worker不启动排队动作；磁盘排空有界 |
| 原连接与批次 | 旧通道资源暂停、断连、新通道重新登记/原分支重复、放行旧动作，新连接只收到自身ID/heads响应且动作一次；平坦重复子项去重，矛盾方向/不支持子项先拒绝 |
| 登记修订 | 新资源安装后完成全修订登记才允许出站业务/转发回调；登记中TC发回调后再PING，PONG证明reader已读到该位置，所选运行的资源动作未提前发生；最后ACK后原回调和单次业务均出现 |
| 实际进程恢复 | 两版本各提交/回滚3子进程，合计12；效果同步后强杀，两个新进程恢复/去重，apply/recover各一次。辅助child入口不另充测试数 |
| 隔离削弱 | 11份D盘副本均编译并在指定目标行为断言失败：原5日志/恢复控制，另Failed/ID/发送标记/清理panic/批次方向/校验期限6项 |
| 静态质量 | RM格式、全target Clippy、rustdoc严格通过；测试覆盖调用的现有依赖组件 |

首个native API RED是缺类型的编译失败。真实生产缺陷RED是关闭收尾吞掉资源Future析构panic，造成shutdown误报Ok；修后保留非取消JoinError并在清理后返回错误。整批期限起点和超限字段先借用校验由源码审查发现，修后两项边界测试与期限削弱检查通过，不虚构修前运行RED。首次native运行另有测试目录时钟碰撞，Atomic计数修复的是夹具，不算生产缺陷。

动态资源的初始试验测试错误要求TC在登记ACK前等待RM回调，违背已批准的only-after-ready契约。临时入站泵循环已逐字节撤回，最终 `connection_pool.rs` 与foundation235728冻结版SHA一致；不是产品缺陷修复，也没有修改核心就绪标准。最终PING/PONG用作reader进度证据，不能夸称dispatcher调度屏障或所有调度下的独立变异认证。

只读终审核对全部catalog/来源/52产物、11真实断言控制、12子进程及四组身份/CRC/序号/状态/效果记录通过；未发现残留测试进程。[审查记录](readonly-review.json)。没有新Java编译/JUnit、真实TC/数据库服务或新Go peer执行；此前独立协议peer235407是历史互操作证据，其72相关编译输入另外保持一致。

该记录只支持模拟TC下的正式RM运行组件，不能关闭真实Java RM重复/重连Gate、SQL/Undo、生产认证、WP00-R、原Go默认差异、完整V1/发布或后续Phase。NotApplied仍须具体资源适配器证明旧尝试围栏；独立sync故障及同租约操作序列单独削弱控制仍待验证。外部TC风险未接受，共享锁问题暂缓。

复现：`python tools/formal-rm-runtime/run.py`。原foundation runner按其历史范围保留，并由新profile复用冻结/依赖/子进程/控制逻辑；在已增长工作区直接运行历史212范围会拒绝测试数量变化，不能把数量不匹配改称成功。执行后发布的README/清单/状态可与历史冻结文档不同；源码/Cargo/依赖单独核验。
