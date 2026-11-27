# 结构化日志计数的认证 HTTP 导出

规格事实源：[WP07 契约](../../wp07-observability-contract.md)中“日志交付计数的认证指标导出”。[机器总账](../../wp07-structured-log-metrics.json)固定最终路径与摘要。本项关闭日志只读计数接入的选定缺口，不关闭完整 WP07、V1 或 Phase0–7。

## 正式行为

structured_logs 特性下，MetricsSources::with_structured_logs 接收现有只读 sampler，既可单独成为来源，也可与 TM/RM/MySQL 来源组合。固定输出 accepted_total、received_total 和 dropped_total{reason=full|closed|oversize|invalid} 共六条 seata_structured_logs_ 序列，没有身份、资源、请求编号、路径或凭据标签。不配置该来源时保留既有输出。重复配置替换原日志来源，不累加高基数来源。

沿用回环 Bearer 端点，未授权响应不披露计数。渲染只读取原子计数，不消费队列、不安装 subscriber、不新增日志递归或数据库/网络调用权限。指标源和 sampler 只保留计数；订阅器/消费者销毁后可读最终累计，也不会延长队列寿命。六计数是不同阶段/拒绝原因，不能相加为事务效果、持久化或 TC 接收次数。快照逐字段读取、累计饱和、重建进程/适配器归零。

## 真实验证

Rust 1.99/1.85 各三个真实 HTTP/TCP 目标通过：

- 仅日志来源从零值开始，真实发布入队/满队列/超预算/非法额外字段，观察入队与消费分离，再关闭消费者产生 closed。最终六计数均为 1；HTTP 认证、六条固定序列、敏感哨兵缺席与重复采集不改变计数均核对。
- 指标源和 sampler 保留时，最后发送器销毁后接收器仍立即结束；销毁日志对象和外部 sampler 后，端点仍读出 accepted=received=1 的最终值。
- 真实 TM 向受控 TCP 对端发出两个原 Status 请求并取得 Rollbacking。单槽日志队列实际接纳一次、丢弃一次；端点同时输出原 32 条 TM 序列和六条日志序列。原 observed=2 与实际请求编号一致，第一原 JSON 对应第一请求；消费后 received 从 0 变为 1，其余阶段保持事实。

原测试曾令 oversize/invalid 同时为 1，无法识别两者互换。最终新增“oversize=1、invalid=0”的分离采样；各阶段向量使任意两种计数的观察不再完全相同。三个正式源反例（省略导出、full 错接 closed、invalid 错接 oversize）双版本共六次在真实 HTTP 断言失败。精确恢复原生产字节后最终目标通过。独立 Python 核对所有原 HTTP、计数和实际 TM 请求事实，每版本九项证据削弱拒绝，包括错接 oversize/invalid。

最终全工作区各 760 passed / 347 ignored（161 个测试/文档测试摘要），观测组件全目标全功能、默认、仅 structured_logs、仅 mysql 严格 Clippy 通过，默认测试通过。默认与仅日志的 normal 依赖树无 SQLx。ignored 不视为通过；本节三个目标已显式执行。本轮不新增原 Java TC 或真实 MySQL 运行结论。

## 诊断与状态

真实 RED 为日志来源启动成功但六条序列缺失。早期四次源反例和第一次回归/验证在计数分离覆盖补充之前，仅保留为历史现场；机器总账指定的最终现场包含独立 oversize 采样。一个回归脚本列表缺括号在任何 Cargo 命令启动前被 Python 拒绝，原脚本及诊断保留于 diagnostic-script-brackets；修正后完整重跑，不把那两次调用计测试执行。

接口 README 已在最终输入冻结前补齐并保留原未改行的换行字节。最终全部归档输入匹配当前字节，生产 metrics_sources.rs 精确恢复，未改变 Cargo 依赖/默认功能或原 JSON/Undo 语义。原 HTTP、源事实、命令、退出码、源码归档、最终严格检查/默认树及校验器副本保留。独立程序不是外部人工/另一 Agent 审查。

生产抓取和告警规则部署、远端认证、sink/保留、应用日志组合、跨进程追踪、其他状态事件、安全/负载和后续阶段仍待完成。

## 复核

按机器总账 rust199、rust185、source_mutations 路径及 catalog 摘要执行 tools/formal-source-junit/validate_structured_log_metrics.py，参数 --rust199/--rust199-sha、--rust185/--rust185-sha、--mutations/--mutations-sha。校验只读旧现场，新结果及自身副本另存。

- red: [red_/20261127_175103_54d80db8](red_/20261127_175103_54d80db8/catalog.json)，SHA-256 `9be25f1a413a07acb6b3ee1543ce5618ebb13a915c1ecf5eb624bf02dda3bd28`。
- rust199: [rust_/20261127_175853_a52ff787](rust_/20261127_175853_a52ff787/catalog.json)，SHA-256 `a8b7a1752dbde0808b2dca2d5bdc74cb175e3503966cd6e242199a4f999b2017`。
- rust185: [rust_/20261127_175858_2b9e7d0b](rust_/20261127_175858_2b9e7d0b/catalog.json)，SHA-256 `7737e1c9b8d83ddd9db5c1707143129ae747357750a3f1e02c0410580b3f892f`。
- source_mutations: [mutations_20261127_175737_422ec753](mutations_20261127_175737_422ec753/catalog.json)，SHA-256 `a7c8b637633b888c27a9b2ebbfe527e83b3f366ce23cce3748e334fecadcfde6`。
- raw_validation: [validation_20261127_175927_7a43e1d4](validation_20261127_175927_7a43e1d4/catalog.json)，SHA-256 `627e02bbbd774a9cc91fbf0fbe9cab4bb9a2746ba311f7da1426806ad78cb1e8`。
- default: [default_20261127_175853_6c19e0e9](default_20261127_175853_6c19e0e9/catalog.json)，SHA-256 `5764c0a9dcf59f6c177f2d687a8e320d35cce1d2f78075cb8b3b165726a504a9`。
- regression_199: [regression_1.99_20261127_175853_a1aea0ba](regression_1.99_20261127_175853_a1aea0ba/catalog.json)，SHA-256 `6fdaa3decf4accbd5cf445408bb0892100341aea757f6cc386fbb60754af4e7a`。
- regression_185: [regression_1.85_20261127_175853_7b44bae9](regression_1.85_20261127_175853_7b44bae9/catalog.json)，SHA-256 `d87bdf53ce76b826eb1e2808947eda8fff05664e1f874c496e54d00237593ef0`。
