# 拟确认的 V1 首期协议范围

状态：Accepted，2026-11-13用户明确确认“批准修订版及首期范围”。本文具体化现行清单§5的V1边界和§7的消息allowlist，不削减完整架构或后续阶段。

V1产品为Java TC兼容的Rust TM/RM，首个真实资源执行器为MySQL AT。协议组件先于连接、TM/RM和资源执行器交付；完整V1验收仍须后续所有组件及故障矩阵通过。XA/TCC/Saga、Rust TC、Java V2和其他传输沿用现行后续阶段安排，不从总体目标删除。

| 首期协议边界 | 明确范围 |
|---|---|
| 原生帧 | Java/Seata V1 TCP，16字节固定头及可选HeadMap；保留请求ID和头字段含义 |
| 载荷 | SEATA serializer=1，NONE compressor=0；其他未实现编码器/压缩器明确拒绝，不能猜测或自动回退 |
| TM事务 | GlobalBegin1/2、GlobalCommit7/8、GlobalRollback9/10、GlobalStatus15/16、GlobalReport17/18 |
| RM事务 | BranchCommit3/4、BranchRollback5/6、BranchRegister11/12、BranchReport13/14、GlobalLockQuery21/22、UndoLogDelete111 |
| 身份消息 | RegisterTM101/102、RegisterRM103/104；V1响应依据identified判断，不能假设带普通事务ResultCode |
| 聚合 | MergedWarp59、MergeResult60，逐项关联语义及正常/拒绝测试；不能因原Go缺codec取消该目标 |
| 心跳 | 空body的request type3/4；不额外塞入普通120载荷前缀 |
| 尚未确认的消息 | UnregisterRM105/106、BatchResult121保持显式未支持/待研究，不能称为已经兼容；若后续验证证明V1正确性必需，则补齐后方可通过完整V1验收，不凭本文永久排除 |

域枚举与线缆值用显式映射，Java状态0–20及已确认的分支/结果/异常码按原source contract表示；协议层能表示一种模式不等于对应RM业务执行器已实现。未知码和已知但未支持的功能均返回明确错误，不能降为AT或Unknown。Java-nullable字段逐消息保留，GlobalReport的255缺省与状态0不同；注册缺省version的原始线缆信息与Java getter默认值分别处理。

Java为语义权威，Go为工程参考。原Go失败长度、异常码分叉及合并缺口保持记录，显式适配需要自身证据；不能把破损默认字节改称Java golden或静默换方言。完整Go兼容和所有既定退出条件继续验收。

严格拒绝截断/尾随数据、非法UTF-8、非法长度/枚举/布尔值和超过配置预算的输入。帧大小与聚合深度/数量等预算须成为正式组件的显式配置及测试，不将探针既有8MiB/深度8等值自动认定为批准的生产API。

本次允许的仅是上述协议范围与实施顺序，不能视为对外部TC缺陷的风险接受、正式组件验收通过或发布授权。原恢复风险条件移至后续完整退出门禁后仍须满足；共享锁保持延期且不计通过。
