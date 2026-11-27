# 有界单文件输出器的双版本证据

规格事实源：[WP07 契约](../../wp07-observability-contract.md)；机器入口：[交付 ledger](../../wp07-structured-log-file.json)。选定组件通过不代表完整 WP07 或 Phase 0–7 已完成。

正式 `structured_log_file` 默认关闭，接收调用方独占的空普通文件与唯一 Receiver。组件不创建路径、截断、轮转或重放。按原字节写入并 flush，真实 sync_all 成功后才确认同步前缀。字节预算、条数/周期/最终同步、脱敏错误、Pending 后原任务等待及重复终态观察已实现；Drop 只请求停止，取消返回 Unknown。操作系统同步确认不等于断电、目录持久化或 TC 接收。

## 最终验收

- Rust 1.99、1.85 各 10 项原生文件集成测试、4 项受控 worker 测试、1 项真实 MySQL 场景通过。受控 Future 只证明停止/取消等待不替换原 Future 或 JoinHandle，不冒充真实阻塞磁盘试验。
- 真实 Windows 只读空文件测试执行系统 sync_all，得到 PermissionDenied/OS 5、零同步成功；只读写入失败可能在 Write 或 Flush 暴露，保留未知尾部标记。非空/位置不为零的文件保持原内容并被拒绝；预算拒绝下一条前同步已有完整前缀。
- 真实 MySQL 使用固定 Java 原 Undo DDL、MySQL 8.4.13 镜像和自有回环容器。插入 fence、读取既有 fence、关闭池后恢复 Unknown 的三条原事件通过标准 Layer 进入正式文件 sink。每版本 received/written/synchronized 均为 3、sync_operations 为 2；独立 SQL 查得 branch 4242/status 1，自有容器已删除。测试 helper 没有代替生产输出器写文件；本轮未重跑原 Java TC。
- 全工作区每版本 780 passed / 348 ignored，164 个成功测试摘要（含 rustdoc）；348 个条件测试没有当作已执行。观测组件全功能、默认、仅日志、仅 MySQL、仅 Layer、仅文件严格 Clippy 均通过。默认/日志/文件正常依赖树不含 SQLx 或 tracing-subscriber，Layer 正常树不含 SQLx，Cargo.lock 与上一提交原字节一致。
- 五类生产源码反例 × 双版本共 10 次被目标行为测试拒绝：删系统 sync、删最终 sync、删字节预算、删批量同步、丢写入失败尾部标记。每次恢复原生产字节。独立校验器还对每版本 10 类伪造证据执行拒绝检查，并核对最终归档源码与当前全部 crates/Cargo 输入的字节一致。

## 固定入口

- rust199: [20261127_190221_a624389e](rust_/20261127_190221_a624389e/catalog.json)，catalog SHA-256 `aedc6fe0b4f3fd243809e0d9c8da36923e3bc09578a1025d03d03292cb7aa302`。
- rust185: [20261127_190221_8b8bb66d](rust_/20261127_190221_8b8bb66d/catalog.json)，catalog SHA-256 `0f5ddde5b9ad4df84307923b5d203d332fca33234dc2166482d96e78b487a19d`。
- mysql: [mysql_20261127_190221_b6916061](mysql_20261127_190221_b6916061/catalog.json)，catalog SHA-256 `6bd4660905438f61ffa39a96409f46dfaee5bbe1685ee8efc2b3b685613d25eb`。
- mutations: [mutations_20261127_190135_b457982a](mutations_20261127_190135_b457982a/catalog.json)，catalog SHA-256 `0f8dcea7e3337b379199eae1f3cf9da1732a69ea04a950371fad41becf885af0`。
- regression199: [regression_1.99_20261127_190226_3ecf4dd9](regression_1.99_20261127_190226_3ecf4dd9/catalog.json)，catalog SHA-256 `028bf13df028f12d0a748b4915746e8f320b1c74ab05f63e7b6ba9a0bb36510c`。
- regression185: [regression_1.85_20261127_190226_077d2e8e](regression_1.85_20261127_190226_077d2e8e/catalog.json)，catalog SHA-256 `9e41d68d4ecf90f81478d16ffb441492919e4f486663a1dff35b5d7c01657279`。
- default: [default_20261127_190221_dd04db43](default_20261127_190221_dd04db43/catalog.json)，catalog SHA-256 `5645416311402af8a0f6a69963349c40413e62e116606adb33439453cad72c19`。
- validation: [validation_20261127_190350_76f59f1d](validation_20261127_190350_76f59f1d/catalog.json)，catalog SHA-256 `fc0f871b6ca7eb36047f518b1850d0a28bd2906ac47c3b6fe6deb70cf0263bb9`。

每个目录保留命令、原 stdout/stderr、输入归档与逐文件哈希。`tools/formal-source-junit/structured_log_file_*.py` 重建对应测试，`validate_structured_log_file.py` 通过 ledger 所列目录与固定 catalog 哈希独立验证；不能只凭 result.json 或测试计划宣称成功。

## RED 与诊断

`red_/20261127_185426_4e976764` 是真实行为 RED：删除系统同步时，只读空文件错误报告成功，命中明确断言；补齐 syscall 后实际系统错误被正确分类。此前测试夹具曾用 i64::MIN 请求 ID，被既有 i32 协议范围拒绝；修正为 i32::MIN 后保留原协议规则。早期 Clippy 拒绝测试中的 expect，已改为显式错误分支。

早期 `mutations_20261127_190035_6591778e` 只完成第一反例，不能计入最终十次。其第二变体绕过最终同步后，任务在零预算观察时已经完成，因此“零预算必定 Pending”的测试假设不成立。最终原生测试允许立即完成或 Pending，并独立校验最终同步；受控 worker 另行保证真正 Pending 与取消等待仍持有同一 JoinHandle。最终反例改用完整写入但未最终同步的前缀断言。

除上方固定最终入口外，其余 rust_/regression_/mysql_/mutation_ 目录为历史 RED、诊断或中间版本，不作为最终源码验收。部分失败诊断没有完成 catalog，保持原输出，不补写成功结果。最终校验器和 ledger 只使用固定最终目录。

生产轮转/保留、目录权限部署、跨进程采集/追踪、断电耐久与外部审查继续开放；完整 WP07 和后续 Phase 4–7 均未完成。
