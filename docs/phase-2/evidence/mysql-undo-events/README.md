# 独立 MySQL Undo 调用的结构化结果事件

规格事实源：[WP07 可观测性契约](../../wp07-observability-contract.md)中“独立 Undo 结果事件”。[机器总账](../../wp07-mysql-undo-events.json)固定最终目录与摘要。本项不关闭完整 WP07、V1 或 Phase0–7。

## 实现与口径

rollback_undo_log 与 fence_undo_log 每个有效身份的已轮询调用输出一个 INFO mysql_undo_outcome（target=seata_db_mysql::undo）。固定字段 operation、xid、branch_id、resource_sha256、outcome、observation、commit_attempted，不含 SQL、Undo、原错误、资源 URL 或凭据。资源不能生成规范标识时摘要为空字符串，不伪造资源；非法身份与未轮询 Future 无身份事件，既有指标仍按原语义结算。

completed 只表示原 API 返回 Ok，observation 是 consumed、fenced_inserted、fenced_existing 或 undo_present；未知提交、提交前错误及取消一律 not_observed。commit_attempted 只在原两个实际 COMMIT 尝试位置标记，不由 operation 猜测；已有围栏及 Normal 存在观察为 false。true 不证明发送或提交确认，服务器已提交但原响应未知时也不发布成功。日志作用域不持连接、原错误或恢复权限，不新增 SQL、重试、持久化或迁移，不改变 Java Undo 格式及旧记录兼容路径。

完成标记在输出前设置，正常返回和解栈不重复发布。单元测试核对非法身份、资源缺席、panic 解栈，以及带账号、密码、查询令牌和驱动错误的上下文脱敏。

## 实际验证

Rust 1.99/1.85 各执行三个独立真实 MySQL 目标：

- 原 Normal 存在、一次原日志补偿、新建围栏和两种只读重复观察。五条事件逐项匹配，业务最终为零行、围栏一行。
- 两个 API 均在 performance_schema 已核实的原 undo_log 行锁等待中取消；关闭原池后再次调用为提交前错误。事件各两条且都未尝试 COMMIT，业务和原 Undo 均保持一行。
- 两个 API 的原 COMMIT 和原服务端 OK 被实际捕获后截留，各测试超时和取消，共四次物理故障/版本。独立 SQL 在丢弃 Future/到期前已看到持久结果，事件仍为 unknown_commit/cancelled、not_observed、commit_attempted=true。同一 XID/BranchId 后续补偿入口新建围栏，围栏入口只读既有记录；不重放业务或第二次补偿。

独立 Python 根据夹具上下文用 hashlib 重算完整资源摘要，核对原 MySQL 包长度、序号、COM_QUERY COMMIT、实际 OK 状态和持久化前后状态；每版本拒绝十一种事件/证据削弱。上下文中资源 URL 和原始包仅供自有夹具取证，不属于产品事件。四种正式源反例（省略完成事件、分别移除两个 COMMIT 标记、取消不发事件）在两版本共八次实际目标失败；精确恢复生产原字节后最终六个真实目标通过。

最终全工作区全功能各 746 passed / 346 ignored（156 个测试/文档测试摘要），MySQL/observability 全目标全功能与默认配置严格 Clippy 通过。默认功能测试通过，默认导出器 normal 依赖树无 SQLx。ignored 不等于通过；本节受选三个 live 目标已分别显式执行。本轮不新增原 Java TC 现场结论。

## 诊断与输入边界

最初 RED 为真实 fence 返回 UndoPresent 后事件数仍为零，不是编译失败。run_20261127_155359_8125f12a 是单元测试新增前的早期双版本现场；run_20261127_155931_eaf53deb 在最终共享测试模块修正前，均不作为当前字节的最终现场。

regression_*_20261127_155931_* 两次工作区测试已通过，但严格 Clippy 报告重复加载共享日志捕获模块，不能当完整门禁通过。现把原捕获文件移到 lib.rs 中唯一 cfg(test) 模块，两个单元测试用 use 共享；沿用原测试作用域对 Mutex unwrap 的许可，不抑制 duplicate_mod，也不修改捕获器或生产行为。修正后的完整双版本工作区与严格门禁重新执行。源反例的两个被改生产文件保持精确恢复；最终 native 全部 Rust 输入匹配当前字节，没有最终源码漂移豁免。

原始运行命令、退出码、输入归档、执行文件摘要、事件、MySQL 原包、SQL 观察及自有容器清理记录均保留。独立程序复核不是外部人工/另一 Agent 审查。其他数据库/一阶段/持久分支事件、生产 subscriber、采集告警与跨进程追踪、生产认证安全和负载门禁，以及 WP05–07/Phase4–7 仍待完成。

## 复核与权威摘要

用机器总账中的 native/source_mutations 目录和 catalog SHA-256，执行 tools/formal-source-junit/validate_mysql_undo_events.py 的 --native/--native-sha、--mutations/--mutations-sha。校验器只读原证据，新结果及自身副本另存。

- red: [red_20261127_155247_4f32258c](red_20261127_155247_4f32258c/catalog.json)，SHA-256 `ad6ed2e941ad7545bcadda91272d74af7e6ed3f426641383c2b5d7a40c577d26`。
- native: [run_20261127_161120_3038dc4a](run_20261127_161120_3038dc4a/catalog.json)，SHA-256 `43db752710fd5555e3875c11943ac1adcd5bc2859fb0f645d35924d343a221e2`。
- source_mutations: [mutations_20261127_155558_495ce59a](mutations_20261127_155558_495ce59a/catalog.json)，SHA-256 `304146d591d1085ce38c4a8e99ab18726ea3880c329206c1d3b6793ec497b569`。
- raw_validation: [validation_20261127_162242_e01b9ea1](validation_20261127_162242_e01b9ea1/catalog.json)，SHA-256 `44fb74aab99182f8e1e14b7ff6552e0798cfddbbb67eae7479ffb5caa78492a7`。
- default: [default_20261127_162204_c41847ce](default_20261127_162204_c41847ce/catalog.json)，SHA-256 `fd5977a409fc114a06a7546670fb9ba6daa4e69e27f7160a6488d422105070f0`。
- regression_199: [regression_1.99_20261127_161120_a1dc40f4](regression_1.99_20261127_161120_a1dc40f4/catalog.json)，SHA-256 `1861374c0f3a000157f1a43039c5e1aa34fe3f0c205fad65b12c2ba75d776a6a`。
- regression_185: [regression_1.85_20261127_161120_67c31a58](regression_1.85_20261127_161120_67c31a58/catalog.json)，SHA-256 `b9f15560e1040a6cd8df702b5dfd98ed95948ad766edee4e80d1f214eaa158fb`。
