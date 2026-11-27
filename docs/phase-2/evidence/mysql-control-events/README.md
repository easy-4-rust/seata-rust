# MySQL 恢复控制的结构化结果事件

规格事实源：[WP07 可观测性契约](../../wp07-observability-contract.md)中“MySQL 恢复控制结果事件”。[机器总账](../../wp07-mysql-control-events.json)列出最终路径及摘要。本项不代表完整 WP07、V1 或 Phase0–7 完成。

## 正式行为

原 MySqlDataSourceManager::control 的已轮询调用建立唯一事件作用域。INFO target 为 seata_db_mysql::control，消息 mysql_control_outcome；字段仅 operation、requested_phase、xid、branch_id、resource_sha256、outcome、resolution、commit_attempted。资源输出完整 SHA-256，事件不记录 URL、SQL、Undo、锁键、application_data、凭据、原始错误或帧。

六种 operation 沿用原控制动作；requested_phase 来源于原动作/回调，表示请求方向，不能当作数据库当前持久方向。completed 表示原内部控制结果 Ok，可能是只读释放或读取既有终态；Applied 的重复观察不算新增业务效果。unknown_commit、before_commit_error、cancelled 与 not_observed 保留原结果语义，不能推断旧业务未执行。commit_attempted 在原可能发送控制 COMMIT 的唯一位置标记，不能从动作名称猜测；相同 Rollback/Commit 动作读取已有终态时可以为 false。标记 true 不等于字节已经送达或 COMMIT 确认。

取消/解栈结算一次并保留已到达的尝试位；正常结算先标记 finished，避免 subscriber panic 后再生成一个取消事件。非法身份在 control 前拒绝，未轮询不生成事件。没有新增数据库查询、业务操作、权限、重试或持久化格式；应用仍需配置 subscriber、采集、权限、保留及追踪上下文。日志不是恢复/提交授权。

## 执行证据

- 双 Rust 各三个真实 MySQL 目标：真实登记/补偿与重复终态读取；实际 InnoDB 行锁等待后取消、关闭池后明确错误；原 COMMIT 命令及真实服务端 OK 被自有代理截留，分别等待期限或取消。原控制写入已独立可见，事件仍 unknown_commit/cancelled 且 attempted=true，随后同一分支补偿完成。每版本两次原 COMMIT OK 故障，不伪造 ACK。
- 事件字段、请求方向和摘要逐项校验；独立 Python 从夹具上下文重算资源摘要并解析原 MySQL 包长度、命令、序号和 OK 状态，核对独立 SQL 观察的前后控制状态及业务行数。上下文中的测试资源 ID 只作摘要对照，不属于产品事件。
- 单元用例验证 panic 解栈只结算一次，以及含用户名、密码、查询令牌和应用数据的资源/错误上下文不会进入事件字段。
- 固定原 Java TC cba73e180fd8896a99df6415b859d275122feaa7，在双版本分别执行正常 Commit、Commit 回调成功响应丢失、Rollback 回调成功响应丢失；原判据、原请求/响应身份及一次业务效果保留。控制事件与停止后原六动作指标逐项配对；原 Callback 帧关联 XID/BranchId/资源和方向，每 profile 的 12 项证据削弱均拒绝。应用恢复控制日志不提供原回调请求编号，因此不伪造该字段，关联须同时使用 RM 原请求日志/帧。
- 三种正式源反例（省略正常事件、移除实际 COMMIT 标记、取消时不发事件）各在两版本目标断言失败；精确恢复原字节后目标通过。

所有数字及最终回归/严格检查结果见机器总账。原始输入、运行命令、退出码、源码归档、实际执行文件摘要、事件、原帧、计数及自有运行时清理事实均保留。

## 诊断与边界

最初 fixture JSON 参数类型编译错误不算 RED；真实控制登记成功而缺事件的断言才是 RED。一次 COMMIT 标记反例先遇到 800ms 夹具等包超时，没有到达目标断言，未计入反例通过；仅把自有故障夹具明确操作预算设为 4 秒、等包为 3 秒后重跑，不修改生产默认期限。

新日志采集曾误扩大旧计数 oracle 的适用场景；现仍只对原 commit_db_drop_ok/rollback_db_drop_ok 调用旧 oracle，新场景由独立事件/指标/原帧校验器验证。保留该次诊断和清理事实。补充采样文件若已由旧 oracle 写入，则核对 JSON 相等，不覆盖原文件。最终指标采样移到 RM/TM 停止后，避免漏掉停机中的控制结算。MSRV 对取证包十六进制拼接的 lint 已修正，并以最终同一源码输入复跑。

数据库其他路径的完整事件、生产 subscriber/采集/告警、跨进程追踪、部署认证、独立外部审查和完整安全/负载门禁、剩余 WP05–07/Phase4–7 继续开放。这里的“completed”不认证业务提交、全局终态或日志已可靠落盘。

## 复核

按机器总账的 native、original_java_tc、source_mutations 目录与 catalog SHA-256 运行 tools/formal-source-junit/validate_mysql_control_events.py 的 --native/--native-sha、--java/--java-sha、--mutations/--mutations-sha。校验器只读原现场，新结果及自身副本另存。所有生产与夹具源输入均须匹配当前字节，本项没有源码漂移豁免。

最终双版本全工作区各 744 passed / 343 ignored（含 rustdoc），受影响 MySQL/observability 全目标严格 Clippy、默认构建测试/严格检查及默认导出器无 SQLx 运行依赖检查通过。忽略项不视为自动通过，本文受选 live 用例已单独执行。

权威 catalog：

- red: [red_20261127_145255_2b4816eb](red_20261127_145255_2b4816eb/catalog.json)，SHA-256 `771f3af6b2a78a9cefd9037c6879164d868d5faeb2a5ae23fe673664da2d7cd6`。
- native: [run_20261127_153804_cd5a4ec2](run_20261127_153804_cd5a4ec2/catalog.json)，SHA-256 `f742634920b671c8f2823fc6af8417390a120d3fd37a561e19139033d4b2c811`。
- original_java_tc: [java_20261127_153804_5be1c0cc](java_20261127_153804_5be1c0cc/catalog.json)，SHA-256 `5d5c59077312ef7796786d6e78249fac22f1c94bbbc827cb67a95b323716e97f`。
- source_mutations: [mutations_20261127_152506_38e763f4](mutations_20261127_152506_38e763f4/catalog.json)，SHA-256 `2eb82710f60bf92f3618c5334416e0b556738b9841fc8adab6c985ff5025cf4b`。
- raw_validation: [validation_20261127_154742_1c3c4dde](validation_20261127_154742_1c3c4dde/catalog.json)，SHA-256 `2a6058856edb8ef1a4229311342b245f651934d8cf7e7bdb973ba7c45ddcd056`。
- default: [default_20261127_154033_33554c6a](default_20261127_154033_33554c6a/catalog.json)，SHA-256 `d36fdff76a94661d12c6a62276cbc141778483cd92bc45afc2195c48c22fda95`。
- regression_199: [regression_1.99_20261127_154033_e0611eb2](regression_1.99_20261127_154033_e0611eb2/catalog.json)，SHA-256 `a756cb3aeef4345a8c4b3126520952389f2b90818f464a14d8a1334ad67f814a`。
- regression_185: [regression_1.85_20261127_153804_9b7069c9](regression_1.85_20261127_153804_9b7069c9/catalog.json)，SHA-256 `f4a73e69ebb0297b9bd2067b02d08f8eada716238ca3aa26d6a1802ab93d8956`。
