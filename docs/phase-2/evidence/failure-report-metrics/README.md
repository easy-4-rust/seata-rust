# 失败报告的分层计量

对应规格：[WP07 可观测性契约](../../wp07-observability-contract.md)中“失败报告分层计量”。[机器总账](../../wp07-failure-report-metrics.json)保留当前权威路径和 catalog 摘要。这里只验证选定观测子项，WP07、完整 V1 与 Phase0–7 仍未完成。

## 正式行为

沿用数据源一阶段只读采样器，增加 rollback_and_report_failure、prepare_failure_report、prepare_and_report_failure 三阶段。区别本地回滚未确认、无分支跳过报告、成功报告响应、报告未知、其他报告错误、前提不适用、已尝试门闩拒绝及准备失败未报告。组合准备成功仍分别记录有无真实登记回执。报告响应不证明业务或全局成功，已尝试拒绝不计本次网络未知尝试。

真实 Enrollment/Undo 错误的私有报告门闩携带只读计数能力；错误离开管理器或数据源关闭后仍归属原计数作用域，不持有池、身份、报告或恢复权限。没有这份来源的本地/登记进度错误不伪造数据源来源，组合入口可记录未报告。未轮询不计，进入后取消/panic 单独结算，原错误槽与门闩保持原义。Checked 转发在 Prepared 底层结算一次；嵌套阶段不能相加当作事务/业务效果次数。

原七阶段 39 条序列保持原义，增加 24 条为 63 条固定序列。无身份、SQL 或凭据标签；默认导出器仍无 SQLx 运行依赖，实验报告 API 默认关闭。公开指标枚举增加变体，穷尽匹配消费者需更新；原变体数值位置保持。本轮不修改数据库结构、Java Undo 格式、原请求可选字段、返回值、错误分类、回执或 TC 重试策略。

## 实际执行

- Rust 1.99/1.85 各八个真实 MySQL/TCP 目标，覆盖 11 组实际计数快照：回滚后报告成功、丢响应、杀原数据库连接后不报告；真实 Undo 唯一键冲突生成的原错误报告成功/明确拒绝/丢失/取消及重复门闩拒绝；组合报告取消保留原错误槽；组合成功报告；无分支/非法身份/未轮询；已登记组合与 Checked 转发。业务、Undo 及原回执分别核对。源关闭后旧错误的重复调用仍被拒绝。
- 每版本一组真实 Bearer HTTP，校验错误令牌及关闭后的准确 63 条序列和敏感哨兵缺席。实验失败报告阶段在这个 HTTP profile 中保持零；非零失败结果由上述现场与原 Java TC 快照验证，不将零值说成已启用能力。
- 固定原 Java TC cba73e180fd8896a99df6415b859d275122feaa7，每版本八场景：已准备回滚报告/丢回包、Undo 约束错误/丢回包/真实唯一键冲突、归属登记错误/真实唯一键冲突/丢回包。真实 TC 每次只见一个失败报告，一个原分支登记和一次原 GlobalRollback。
- 独立校验解析原 V1 注册请求/成功回包，关联 TC 实际签发 BranchId、原 XID/资源/可选字段与 status=3 的失败报告，核对同连接/请求编号及成功报告原回包确实送达或丢弃。每 Java profile 的 12 项证据削弱均拒绝，包括连同汇总身份一起改写 BranchId 的伪证、重放、错状态和假送达。
- 三个正式结算源代码变异在每版本均导致目标计数断言失败，精确恢复原字节后现场通过。校验器不仅查退出 101，还根据归档源码确认 panic 位于原 snapshot(operation).count(outcome) 断言，防止把无关 fixture 错误算作反例。
- 两版本全工作区全功能各 743 passed / 340 ignored，含 rustdoc；受影响 MySQL/observability 全目标严格 Clippy、默认测试/严格检查和依赖检查均通过。忽略数不能当通过数，本项受选 live 测试已另外实际执行。

## 输入绑定与诊断

最终 native 源码输入与当前文件逐字节一致。Java 执行后的唯一源差异是共享 native 测试助手 save_phase_metrics 增加一行 allow(dead_code)，供未调用它的其他测试编译单元通过严格检查；无运行逻辑或生产源码变化。只读校验器将归档原字节、当前字节与该精确单行增量逐字节核对并记录两侧摘要，不允许其他源码漂移。此后 native、全工作区与默认严格检查均已重跑通过。

最初新增结果数量超过数组 Default 派生支持范围，已用显式零值 Default 支持两个编译器；该编译失败不是 RED 证明。之后真实原报告成功但阶段计数为零的断言才是权威 RED。早期 Green 因输出路径未使用 D 盘被夹具拒绝、所有权编译失败及测试助手 dead_code 诊断均保留，不计最终通过。

第一次 Java 矩阵保留 15 个通过场景，最后一个 owned RM 代理因 Windows 10013 无法绑定端口，尚未进入客户端测试；所有自有进程/容器清理。最终重新选择端口的完整 16 场景全部通过。未修改 OS 端口策略或重启一个尚在运行的旧任务。

程序校验不替代独立外部审查。数据库完整结构化事件、生产采集/告警、部署认证、完整安全/负载门禁与剩余 WP05–07、Phase4–7 开放。

## 权威 catalog

- red: [red_20261127_071602_7ff6dddb](red_20261127_071602_7ff6dddb/catalog.json)，SHA-256 `f1807031d4cda766f7e377456815e5fe038afa3dea5c0ff4fb32cc1a547411fb`。
- restored_native: [run_20261127_143120_6e594ba9](run_20261127_143120_6e594ba9/catalog.json)，SHA-256 `5bf6ebcae90fb1fbc795a56db371157fc24ec6d5e9bc2bd253075ad22a13f2fb`。
- original_java_tc: [java_20261127_142210_f159c2dd](java_20261127_142210_f159c2dd/catalog.json)，SHA-256 `bffdaafcdeeda1121f84be04904541dca7ac4399f664e095a121f9084ab365fb`。
- source_mutations: [mutations_20261127_141929_138573af](mutations_20261127_141929_138573af/catalog.json)，SHA-256 `d53f3440acacccc0bb42ac0e61e41a198281ea1d4a2a30420b2a529c03f41cc1`。
- raw_validation: [validation_20261127_143748_04a090ae](validation_20261127_143748_04a090ae/catalog.json)，SHA-256 `d30a1a0769d091d1bd970d48bb3f2b27aeea60ea1ca7a2a1cf3eb12f2085d8fd`。
- default: [default_20261127_143308_36b6791d](default_20261127_143308_36b6791d/catalog.json)，SHA-256 `298774ca60abc87ee3e896476e94cf34f9bdd9fdc6464f85c5b4ea9d3f45efbf`。
- regression_199: [regression_1.99_20261127_143308_ca310a84](regression_1.99_20261127_143308_ca310a84/catalog.json)，SHA-256 `aeb2834a024c62218e15a7a3c67b723292d6867c0589fba5155cc4c6aba01f36`。
- regression_185: [regression_1.85_20261127_143308_39d3a099](regression_1.85_20261127_143308_39d3a099/catalog.json)，SHA-256 `38ae580dda51ad48541e074a3edb09d66599352a617afd594f0f70bffb2aeb61`。

复核运行 tools/formal-source-junit/validate_failure_report_metrics.py，分别将 restored_native、original_java_tc、source_mutations 三组目录和摘要传给 --native/--native-sha、--java/--java-sha、--mutations/--mutations-sha。校验器只读原现场，新结果与执行校验器副本另存。原命令、退出码、源码归档、可执行文件摘要、计数快照、原始帧和清理事实保留于现场。
