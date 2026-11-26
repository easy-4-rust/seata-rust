# WP-02 raw连接池与代际重连（2026-11-13）

正式运行：[run_20261113_195040_bb30141c](run_20261113_195040_bb30141c/result.json)。catalog SHA256=`b326692a210b89b945a73b3cd53406de0da0b2ac18fd02195c1aaa9389774586`；2564来源、2599归档成员、10个实际执行测试binary。16registry依赖archive checksum、实际解包源码、所有正式代码/Cargo输入/规格和已有peer引用封存；无新依赖。

| 验证 | 实际结果 |
|---|---|
| 两工具链 | Rust1.99/最低1.85各62通过：19unit＋10pool target（9真实TCP/1配置）＋9heartbeat target（8真实TCP/1配置）＋10stream/ID＋14原TCP，总31真实TCP函数 |
| 物理所有权 | 配置两个物理槽位，100并发acquire只复用两个代际、实际连接尝试总2；正常shutdown后peerEOF且无新连接；Drop清理活动调用/状态，取消未完成connect并唤醒等待acquire |
| 重连和隔离 | 丢失Begin响应保留Unknown，替换socket首个业务请求必须是调用方显式Status；旧handle保持关闭，不迁移代际；IDquota1耗尽后关闭并轮换但拒绝Begin不重放 |
| 退避 | 实际短连接序列观测60/120/200ms上限，稳定600ms（门槛500ms）后恢复60ms；独占state的饱和、零环绕/短连接不重置另有unit |
| 期限/入站 | 相同绝对deadline覆盖acquire80ms＋等待120ms；延迟首次session poll和同步锁等待不延长期限；入站绑定原代际，pool队列满关闭该通道且已入队消息保留旧handle |
| 五项变异 | 单D隔离副本每项从基线重建：隐藏重发、短暂连接重置退避、Drop不立即关闭、忽略acquire耗时重算预算、quota不关旧代际；目标测试均实际断言失败exit101，编译错误不计拦截 |
| 质量 | workspace fmt、全target Clippy -Dwarnings、rustdoc -Dwarnings通过 |
| 协议绑定 | 复用192819已经成功的628/2050、74＋1双版本/7Python/12负控证据；70协议/Cargo编译输入当前一致，所有子归档SHA核对。本轮没有新跑协议peer/Java，不将旧执行计为新执行 |

初始API RED只是缺类型/错误原因的编译失败。复审两P2分别真实RED：quota后旧通道仍开放，导致池永远选择耗尽句柄；同步锁等待60ms后请求仍剩4.999514s，说明期限重新起算。前者改为同步关闭旧代际并返回原本地错误，后者建立/传递唯一绝对deadline；61项GREEN后补关闭建连控制，最终双版本62通过。独立副本的五变异是刻意构造回归控制，不是五个原产品缺陷。

[只读终审](readonly-review.json)逐项核对2599成员/2564输入、10实际产物、双版本日志、16依赖archive及1850包内文件与实际源码快照。五变异SHA由原source+patch重算一致、D副本恢复，未见本轮自有cargo/test/peer残留。既有peer70编译输入匹配，后续approval/checklist文档更新不算编译漂移；审查未重新运行服务/测试。当前正式编译源另行核对，后续README/验收状态更新保留时点差别。

Connected只表示raw TCP，尚未提供认证/缓存资源重新注册ready层。地址发现、TLS、jitter和生产调优仍属后续WP-07；RM回包/业务恢复及NET008–012继续必需。WP02当前10/12、完整Gate仍OPEN，完整V1及后续全部阶段未完成。无新TC/MySQL、Java、Go254/旧probe117运行，未安装/联网、创建或切换分支；共享锁延期与外部TC风险未接受不变。

复现：`python tools/transport-pool/run.py --peer-run docs/phase-1/evidence/protocol-peer-profile/run_20261113_192819_1650e24d`。需要固定D/E来源和现有缓存，只新建运行目录及D隔离副本，不改已封存证据。snapshot记录执行时点；README/验收状态随后更新不称全部旧输入当前零漂移，编译源码另行核对。
