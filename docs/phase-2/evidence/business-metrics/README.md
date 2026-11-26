# 业务 DML 与显式本地回滚计量

规格事实源：[WP07 可观测性契约](../../wp07-observability-contract.md)中“业务 DML 与显式本地回滚计量”。当前为该子项的真实 MySQL 现场，不代表 WP07、V1 或 Phase0–7 全部完成。

## 正式行为与兼容性

原数据源共享的一阶段只读采样器新增 execute、rollback 两阶段。execution_completed 只表示原语句/镜像在未提交事务中返回成功，包含零影响行；execution_failed 保留原错误，不推断业务 SQL 未执行。显式 rollback_confirmed 仅在原驱动返回成功时记录；错误记 rollback_unconfirmed，取消单独记 cancelled。返回/取消均归还 active。未轮询 Future 与自动 Drop 回滚不计显式 API；Prepared/Checked 转发在底层只结算一次。

原五阶段、31 条导出序列保持原义，新增八条，共 39 条固定序列，无 SQL、身份或凭据标签。原有公开 API 返回值、错误、期限、事务所有权及实验有变化提交默认关闭保持原义；公开指标枚举新增变体，使用穷尽匹配的调用方需处理它们。历史 HTTP 测试名称中的 31 保留供既有现场运行器调用，当前内容明确校验 39 条；历史固定摘要的证据校验器仍只解释其旧现场，不回填新结论。

## 新执行现场

Rust 1.99/1.85 各五组真实 MySQL 用例及一组 Bearer HTTP：

- INSERT/UPDATE/DELETE 与零行、未轮询 DML/rollback、自动 Drop、克隆归属、数据源关闭后最终快照；独立连接在 DML 成功后仍见原值，显式回滚后仍见原值。
- 参数不匹配、真实重复键 1062 与中毒作用域拒绝，错误累计与原返回类型一致，不提交先前 UPDATE。
- 独立事务阻塞业务行，通过 performance_schema 实际锁等待核验 active，取消与客户端 Deadline 分开；原值不变。客户端显式期限 1.5 秒，小于正式连接既有 2 秒服务端锁超时，不扩大生产超时。
- Prepared 与 Checked 的两次显式回滚，各只累计一次。
- 自有代理截留原回滚后的服务端字节，独立连接已能锁定原行且值未变；取消或期限后指标仍分别为 cancelled/unconfirmed，confirmed 均零。此处不把被截留字节假称为已解析的权威 OK 帧。
- 认证 HTTP 的零值及关闭后准确 39 条序列；错误令牌 401，敏感哨兵缺席。

首次真实 UPDATE 的缺失计数断言为 RED。两种正式结算源代码变异（DML、rollback）在每版本均触发目标行为断言失败，原源码字节精确恢复后，五组现场及 HTTP 再通过。只读校验器逐阶段读取实际原子计数快照、真实等待/代理字节和原 HTTP 文本，每版本拒绝 12 项证据削弱。独立外部审查保持 OPEN。

权威现场与摘要记录于 [机器总账](../../wp07-business-metrics.json)。诊断运行保留，早期 START TRANSACTION 误走 prepared 协议 1295、服务端先于客户端的锁等待超时、测试夹具 expect lint 失败均不计入最终通过。最终使用 raw_sql 的事务控制、明确较短的客户端期限与仅限夹具的 expect lint 声明。没有放宽生产规则。

## 复核

从仓库根运行 tools/formal-source-junit/validate_business_metrics.py，传入机器总账中的 native/restored_native 和 source_mutations 路径及 catalog SHA-256；该校验不修改原始现场。运行器的 commands.json、原 stdout/stderr、源码归档、实际可执行文件摘要、原子计数快照及自有容器清理结果均已保留。

生产采集/告警、失败报告的完整分层计量、结构化数据库事件、独立审查及剩余 WP05–07、Phase4–7 继续实施。本轮没有新跑原 Java TC，沿用原语义不能当作本轮 Java 联试证明。

两个版本全工作区全功能各 743 passed / 335 ignored（含 rustdoc），受影响 seata-db-mysql/seata-observability 全目标全功能严格 Clippy 通过。335 个 ignored 不视为自动通过；本增量选择的五个 MySQL 目标及 HTTP 已另执行。

默认配置测试与两组件默认全目标严格 Clippy 双版本通过；默认 observability 正常运行依赖无 SQLx。

权威 catalog：

- red: [red_20261127_065646_9ba0db05](red_20261127_065646_9ba0db05/catalog.json)，SHA-256 `a2c9548f8919610855592f325b311159745f5829eb44cab1048f6c0ee5353a7e`。
- restored_native: [run_20261127_070635_1eb289f2](run_20261127_070635_1eb289f2/catalog.json)，SHA-256 `69d8b0c500d83832b2e93911c7864712cf8c646f56953775a69e480fab3882b4`。
- source_mutations: [mutations_20261127_070210_3fd2a6af](mutations_20261127_070210_3fd2a6af/catalog.json)，SHA-256 `b36076602b5b2b57e1291ec16070658e60fa75fd10b75295f717b33402a72ff2`。
- raw_validation: [validation_20261127_070738_8ff96e8e](validation_20261127_070738_8ff96e8e/catalog.json)，SHA-256 `5b3b8ad31a9366919e64550d11cf485cae214be53c6e62d17d13353c37d00684`。
- default: [default_20261127_070823_f36ae67a](default_20261127_070823_f36ae67a/catalog.json)，SHA-256 `921ab3d143a1a84bafc647f6c7761e452adfc8f807916ecd81780d94d87f9ff9`。
- regression_199: [regression_1.99_20261127_070728_1fc1c2f3](regression_1.99_20261127_070728_1fc1c2f3/catalog.json)，SHA-256 `a01a4e4234ce387103d9e8122e2592ac99f523246462769143c29554f66cfc93`。
- regression_185: [regression_1.85_20261127_070728_a3729d2c](regression_1.85_20261127_070728_a3729d2c/catalog.json)，SHA-256 `28dbfd3ad0cb2378ad6660616ff7af5029f3c7e1836b6123dc0841648cec1d19`。
