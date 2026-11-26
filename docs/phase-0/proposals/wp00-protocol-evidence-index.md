# WP-00-P 证据与正式协议实施入口（已启用）

状态：Active。用户已明确批准V2顺序及首期范围，WP-00-P已按启动记录逐项核对。本文仍只索引既有Markdown/ADR事实源，正式组件及完整WP00-R须另取得自己的实现与验收证据。

| P条件 | 可核对的现有权威资料 | 正式协议组件仍须自己证明 |
|---|---|---|
| 固定权威和版本 | [ADR-0007](../../adr/0007-v1-reference-hierarchy.md)、[源码映射](../source-mapping.md)、原Java cba73e180fd8896a99df6415b859d275122feaa7 / Go82570d905919c1c66bb025a3a5979f25390de498 | 记录peer profile，不由RSeata或Go差异重定义Java语义 |
| 消息字段/载荷/原样例 | [原29/6当前认证](../evidence/local-wp00-vector-audit/README.md)、[字段说明](../protocol-golden-vectors.md)、[二进制manifest](../../../tests/fixtures/protocol/v1/manifest.json) | 解码独立对象与从字段编码；29原样例、全部支持消息及失败变体逐字节测试，不缓存原字节冒充编码 |
| 异常语义及失败结果 | [Java请求/异常矩阵](../java-tc-request-exception-matrix.md)、[250异常候选](../evidence/local-go-java-exception-profile/README.md) | Failed保留错误文本/异常码/状态，不能由状态或lockable覆盖；Java19 NotRaftLeader与Go19 fail-fast显式区分 |
| 状态域及转换 | [ADR-0003](../../adr/0003-explicit-wire-code-mapping.md)、[21状态实际契约](../evidence/local-global-status-contract/README.md) | 码值显式双向映射、未知码拒绝；不把Java辅助分类混为统一terminal，也不将数字往返或validator许可称业务成功 |
| 可选字段与结果不确定性 | [已接受ADR-0008](../../adr/0008-wire-optionality-and-unknown-outcomes.md)、[Java可选字段证据](../evidence/local-rust-payloads-20261112/README.md) | null/空逐消息处理，report255与Unknown0分离；wire version与Java getter默认值分离；TM不确定结果的正式实现留WP03 |
| V1帧与身份响应 | [帧布局](../protocol-v1-frame-compatibility.md)、[注册合同](../registration-v1-wire-contract.md)、原Java帧断言 | 长度/头部/序列化器/压缩器、HeadMap与full consumption；注册结果依据identified；V2不冒充V1 |
| 聚合与心跳 | [控制信封合同](../protocol-v1-control-envelopes.md)、[38组合候选](../evidence/local-go-java-merge-profile/README.md)、[原SDK候选](../evidence/local-go-java-sdk-profile/README.md) | 59/60长度/计数/子消息顺序/关联；心跳无普通payload；不把工具封装载荷称Java实际完整帧 |
| 正常、失败与拒绝入口 | [现行WP01九项](../v1-implementation-acceptance-checklist.md)、[已确认allowlist](v1-protocol-allowlist.md)、现有独立Rust测试源码 | 先写失败测试再实现；截断/尾随/未知码/UTF-8/长度和可配置预算检查，执行目标与受影响回归 |

首个正式交付保持原WP01：Cargo workspace内seata-protocol组件，域对象/线缆映射、消息模型、严格原语、原生帧及allowlist载荷，逐项完成原有验收。crate不承担TCP重连、TM重试、RM数据库执行或TC调度。每个Rust对象按一个Java对象/文件组织，中文doc标注来源；不复制RSeata实现，不引入未安装的规格工具。

建议的TDD实施顺序只是细化原任务：显式枚举/错误 → 长度及UTF-8原语 → 事务与注册模型及25普通载荷 → 可选字段/全部失败结果 → V1帧/HeadMap/空心跳 → 59/60聚合 → 三方与畸形输入回归。每一步必须先取得相应失败测试，再实现和执行回归；文件存在或cargo build成功不能勾选WP01。正式组件通过后才按原依赖推进WP02/03，完整资源与恢复验收继续WP04–07及WP00-R。

R仍记录全部未满足范围：原Go默认差异、状态业务语义、空报告、原FILE/RAFT及候选完整配置/故障/发布边界、指标/Exporter、正式资源原子性/Undo/持久重放、生产认证和共享锁延期项。参考Java/Go通过不能转记为正式Rust通过。实际代码与运行结果未满足前，不关闭这些要求。
