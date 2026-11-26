# WP-02定时心跳与读空闲门禁（2026-11-13）

正式运行：[run_20261113_192909_28bdbb1f](run_20261113_192909_28bdbb1f/result.json)。catalog SHA256=`bd05a0602c9e4172e50affc6bd159c186227ea1407b4b19072367598e57caaf9`；2556来源输入、2585归档成员、8个实际测试binary。封存包含所有正式代码、Cargo输入、16锁依赖archive checksum和解包源、五份原始Java心跳/idle参考文件、现有规格及新peer引用。

| 验证 | 实际结果 |
|---|---|
| 正式transport全量 | Rust1.99/最低1.85每套47通过：14unit＋9heartbeat target（8真实TCP/1配置）＋10stream/ID＋14既有真实TCP；总22真实TCP测试函数 |
| 调度与活性 | 空闲后空原生PING3、PONG4维持跨多个读idle窗口、静默peer使已写请求UnknownOutcome/ReadIdleTimeout、持续半帧滴流在首帧完成前终止、共享ID额度到界关闭、默认关闭及ownerDrop后无timer活动 |
| 写入接线 | 连续业务完整写与PONG写跨初始writer窗口抑制本端PING；暂停时钟直接驱动同一生产writer循环，在2/4/6/8ms注入已取消残项，10ms仍应发PING；残项不写入/不刷新、不分配业务ID |
| 单调边界 | 四个IdleTimer paused-time单测核对包含边界、活动刷新、禁用、长时间调度延迟后从当前时间计时而不追赶burst |
| 四实际负控 | 在独立D副本分别故意让TCP字节刷新read、业务写不刷新write、PONG写不刷新write、skip错误刷新write；每个指定测试真实断言失败exit101。编译错误不能计为拦截，正式源码未被负控修改 |
| 质量 | workspace fmt、全target Clippy -Dwarnings、rustdoc -Dwarnings通过 |
| 协议回归 | 新peer192819：74协议＋1验证器在双版本、628正向/2050负向、7Python/12实际输出破坏负控全部新跑通过；catalog=`b8ddd7aac9ee5c776a9268f74bb1ffeec09c7939907aaf905b50438bcf84e9e2`；编译协议/Cargo输入绑定 |

初始API RED为缺公开类型/字段的编译失败，不是七个运行断言失败。四项隔离行为变异控制是故意构造的回归控制，不是原始产品缺陷。实际复用一个D工作副本，每项从相同正式基线重新构造变异，最后恢复该副本；business/PONG两项使用相同删除reset的变异但运行不同目标测试，不声称四个目录或四种不同源补丁。首轮静态复审未发现实现阻断，但指出business/PONG/skip接线测试缺口；补两个真实TCP函数及同一生产writer的暂停时钟单测，并增强持续滴流测试后，通过实际负控和全量双版本验证。私有writer泛型AsyncWrite的生产实参仍为OwnedWriteHalf；没有替换生产路径。

[只读封存终审](readonly-review.json)独立核对全部2585成员/2556来源、8产物、两套47测试日志、16依赖archive checksum和1850包内源码文件、5份Java源与固定pin及新peer70个编译输入；628/2050离线重算通过。四项变异SHA从原来源和补丁重算一致，均实际断言失败，D副本基线恢复；CIM未见本轮自有进程残留。文档状态随后更新，编译源码保持封存一致；未重跑测试/服务。

Java参考源来自固定`cba73e180fd8896a99df6415b859d275122feaa7`，五文件git diff为空；只读来源核对，没有新Java执行或现场TC。Rust策略显式配置，无默认Netty秒数/全部调度细节等价声明；readidle以完整有效帧刷新，片段不刷新，这是防止无限滴流的明确策略。PONG只是控制帧，不证明认证/RMready，也不完成普通pending。

本轮只新增Tokio开发期test-util功能，运行期锁依赖包仍16个，全部离线缓存使用；未安装CLI/Skill或联网下载。没有新旧probe117/Go254执行、TC/MySQL/外部服务、业务自动重放或分支操作。WP02目前8/12；连接池、重连退避、认证/缓存资源重新注册、NET008–012业务恢复、完整V1及所有后续阶段仍开放。共享锁保持延期、外部TC风险未接受。

复现：`python tools/transport-heartbeat/run.py --peer-run docs/phase-1/evidence/protocol-peer-profile/run_20261113_192819_1650e24d`。需要固定D/E来源和本地缓存；工具只创建新运行目录，负控创建独立D工作副本并使用独立D构建目录。归档中的source-inputs记录执行时刻快照，随后README和清单更新不构成编译源码漂移。
