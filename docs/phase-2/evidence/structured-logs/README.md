# 有界结构化日志订阅与 JSON 交付

规格事实源：[WP07 可观测性契约](../../wp07-observability-contract.md)中“有界结构化日志订阅适配器”。[机器总账](../../wp07-structured-logs.json)固定最终目录、摘要及门禁。选定适配器已验证，完整 WP07、V1 和 Phase0–7 继续开放。

## 正式接口与边界

默认关闭的 structured_logs 特性公开 StructuredLogConfig、StructuredLogSubscriber、唯一 StructuredLogReceiver、只读 StructuredLogSampler/StructuredLogStats 和类型化配置错误。组件不会安装全局订阅器、创建线程/文件或联网。应用显式选择 Dispatch 与输出器。

只接纳七种现有正式事件：TM/RM 出站、RM 本地决策/回包、MySQL 控制、独立 Undo 结果和 Undo 摘要拒绝。访问字段值之前核对完整静态字段集合，额外字段连 Debug 都不调用；固定消息、枚举、数值和资源摘要校验失败则整条拒绝。任意应用事件、span 字段、原错误、SQL、Undo、资源 URL、凭据不进入这一导出流。身份 XID 是允许关联字段；应用主动把秘密塞进 XID 本身不在语义脱敏承诺内。

源字段均保留 tracing 原文本形式，尤其 signed request_id、Some/None Debug 表示和大 BranchId；不从文本制造新身份。JSON 信封仅 target、level、timestamp_unix_millis、fields，fields 含原静态 message。时间来自本机墙钟，不授权恢复或推断事务顺序。JSON 转义引号/换行，不截断身份输出残缺记录。

容量 1–4096，含换行的完整 JSON 上限 512–262144 字节，乘积不超过 64 MiB；字段内容与 JSON 编码各自受限。队列内容预算不等于全部 allocator、线程或应用输出器的堆内存上限。事件路径仅 try_send；满时丢弃新记录，消费者关闭后后续事件另计丢弃，不等待输出器。适配器不记录自己的错误，防止日志递归。

accepted 表示本机入队，received 表示取给消费者，均不表示持久化、远端接收或 TC 接收。另有 full/closed/oversize/invalid 四类丢弃累计；六计数饱和不回绕，快照逐字段读取。仅保留 sampler 不延长队列寿命。Receiver.close 停止接纳，随后 recv 可排空旧队列并返回 None。直接 Drop 接收器会丢弃未取记录，不能据 accepted 声称保存完成。

这是专用 Subscriber，不是通用应用日志 Layer。全局安装后只输出列明事件；应用现有日志链的组合、跨任务/进程 span 传播仍需明确集成。已有全局订阅器时安装返回原 tracing 错误，不静默替换。作用域默认值的跨线程传播也不能靠本组件自动获得。

## 实际执行

双 Rust 各十个目标测试覆盖七模板、字段前置拒绝、JSON 字符转义、配置/乘积溢出、两级字节预算、并发八线程向四槽队列发布 2048 事件、满时不等待消费者、关闭后排空、非法值/消息/摘要拒绝，以及真实 TM/RM TCP。

TM 三次真实请求包含成功、TC 明确拒绝和实际丢失回复，日志保持 response/response/unknown。RM 四次包含相同两种回应、回复丢失和关闭后的 not_sent，不编造未分配编号。RM 三个原信封含 i32::MIN、重复提交和反向回滚，记录六条决策/本地写入事件；原 BranchId 9007199254740995 精确保留，资源仅执行一次。本地写入不是 TC 接收证明。

双版本另各一项真实 MySQL 目标：原 Java Undo DDL 上新建围栏、同身份只读既有围栏，再关闭池进行管理器恢复。原内部控制日志为 before_commit_error，但公开 BranchResource 返回 Unknown，二者保留各自语义。独立 SQL 实际确认原分支 4242、log_status=1 的一条持久围栏。自有测试消费者对真实 TM/RM/MySQL 的 JSON 执行 create_new、write_all、sync_all，再按原字节回读；这仅验证自有文件现场，不代表生产输出器已部署。

独立 Python 从原请求编号、原回调响应码、资源标识及数据库 SQL 观察核对文件，每版本十五项证据削弱拒绝。四种正式源反例（跳过前置字段数量校验、改为实际阻塞满队列、移除编码字节预算、跳过资源摘要校验）在双版本共八次目标失败。阻塞反例确实等待满队列，接收器销毁后线程解除等待；不是用 sleep 模拟。精确恢复生产原字节后，全部最终目标与真实数据库通过。

最终全工作区各 757 passed / 347 ignored（160 个测试/文档测试摘要）。观测组件全目标全功能、仅 structured_logs 和默认严格 Clippy 通过，默认测试通过，默认与仅日志特性的 normal 依赖树均无 SQLx。ignored 不等于通过；本节受选 MySQL 目标已显式运行。本轮不新增原 Java TC 联试结论。

## 诊断、依赖与剩余工作

最初行为 RED 是额外密码字段 Debug 被调用一次，而要求为零；访问值前的完整模板校验使同测试通过。最初锁文件生成出现一个无关的 windows-sys 依赖边变化，已从固定基线精确恢复；最终只有观测组件的 serde_json/tracing 可选依赖边增加，未升级任何包。该 RED 现场不是最终依赖输入。

早期 rust_/20261127_170816_8222dd08 与 mysql_20261127_172445_135343b0 是原型现场，不冒充当前字节。一次测试 expect 的严格 lint 在正式封存前改为带明确消息的断言。最初 regression_*_20261127_172911_* 实际命令只检查观测组件，但复制的结果标签误称 MySQL；不作为最终严格门禁。标签修正后，以最终脚本完整重跑两个工作区与观测组件严格检查。

接口 README 补齐后再次执行双版本目标及真实 MySQL；最终归档同时包含相同的接口文档字节。最终 Rust/真实数据库执行输入逐字节匹配当前源码，源反例文件精确恢复，无最终源码漂移豁免。原命令、退出码、输入归档、JSON 文件、源事实、自有数据库清理、默认依赖树和校验器副本均保留。独立程序不是外部人工/另一 Agent 审查。

应用 sink、保留期、文件/目录权限、远端 TLS/认证、采集告警和丢弃指标的 /metrics 接入仍开放；当前只读 sampler 可供应用读取。现有应用日志组合、跨进程追踪、其余事件、生产安全/负载及全部后续阶段继续。

## 复核

以机器总账的 rust199、rust185、mysql、source_mutations 路径及 catalog 摘要执行 tools/formal-source-junit/validate_structured_logs.py；参数分别是 --rust199/--rust199-sha、--rust185/--rust185-sha、--mysql/--mysql-sha、--mutations/--mutations-sha。校验只读旧证据，新结果及自身副本另存。

- red: [red_/20261127_170728_c2d1e653](red_/20261127_170728_c2d1e653/catalog.json)，SHA-256 `055458e3bd5113fa406d0aad3e4355833767b0c4b1ed22615193d9b9d96d41af`。
- rust199: [rust_/20261127_174500_7fe04404](rust_/20261127_174500_7fe04404/catalog.json)，SHA-256 `561f414a406db491b05fa39f531d039e209efe3784fc1bc2d79691d02f74455d`。
- rust185: [rust_/20261127_174501_76e3b176](rust_/20261127_174501_76e3b176/catalog.json)，SHA-256 `9d150badbdb22415a6251e2c8169334f390f5d83dc710d3a4cc2070b2e01c07d`。
- mysql: [mysql_20261127_174500_126bb8ef](mysql_20261127_174500_126bb8ef/catalog.json)，SHA-256 `71b8f7451e28a7d19bb3648dfec6a675743a9d2badbeabfcaf351f9d7583df36`。
- source_mutations: [mutations_20261127_172738_12ad9565](mutations_20261127_172738_12ad9565/catalog.json)，SHA-256 `ed193a545fbf14f4968ceb7323773edb26587459f8858405f3f13ce9c1791006`。
- raw_validation: [validation_20261127_174618_34d2e716](validation_20261127_174618_34d2e716/catalog.json)，SHA-256 `3403a5060539849654c857b4c92398fa623455b54d1211e77c6577380ff4067b`。
- default: [default_20261127_173146_b12ac1f7](default_20261127_173146_b12ac1f7/catalog.json)，SHA-256 `dac1670caa9c4eee2654729cddd9179f2de5452aac203d34f4d6acc47bce8441`。
- regression_199: [regression_1.99_20261127_173231_338c46e7](regression_1.99_20261127_173231_338c46e7/catalog.json)，SHA-256 `d118f0ee1bf250c58cdaa59b5f4935e25b55604cfc39ef49aeee0c00f59420ca`。
- regression_185: [regression_1.85_20261127_173231_d827727a](regression_1.85_20261127_173231_d827727a/catalog.json)，SHA-256 `9c9c031f3eb4e5676248c7fc404aa6f0c5253972120e325ce8cedc7a1e6f2b2e`。
