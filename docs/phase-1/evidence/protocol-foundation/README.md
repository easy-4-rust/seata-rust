# 正式 seata-protocol：枚举与字段原语

用户明确确认“批准修订版及首期范围”后，V2顺序和allowlist已生效，WP-00-P逐项核对完成。正式根Cargo workspace及[协议crate](../../../../crates/seata_protocol/README.md)已建立；原WP-01仅完成域枚举、显式码映射、有界字段原语三项。

正式运行：[run_20261113_161037_76f1561c/result.json](run_20261113_161037_76f1561c/result.json)，[673文件清单](run_20261113_161037_76f1561c/catalog.json) SHA-256 `47152102d5213b6ce309f0c15a00d9c165f5969e075d80891d62b0cf2bf1d980`。650个源/夹具/元数据输入及实际测试binary冻结、前后核对。

| 实际实现/验证 | 结果 |
|---|---|
| 九组协议域 | MessageType、RequestType、ResultCode、BranchType/Status、GlobalStatus、异常码、serializer、compressor；每个对象独立文件，无repr/ordinal转换 |
| 显式转换 | 21全局状态、16分支状态、25异常码、稀疏消息码等逐值核对；未知byte码及全部未知u16消息码拒绝 |
| 严格字段原语 | 大端有/无符号整数、UTF-8字节长度、signed string16/string32、HeadMap nullable、规范布尔、截断/尾随/预算检查 |
| 安全边界 | writer字段失败不留下部分写入；reader错误后不支持恢复；Debug只输出长度/位置，不打印载荷；类型化错误使用thiserror |
| 正式crate测试 | 12个独立测试，Rust1.99与最低Rust1.85分别全部通过；不是24个独立功能测试 |
| 质量检查 | fmt、全部target Clippy拒绝warning、rustdoc拒绝warning通过 |
| 原独立探针 | 新执行117测试、fmt/Clippy通过；根workspace显式exclude维持原manifest命令，未把探针当正式crate交付 |

RED日志分别是公共API/字段原语尚缺实现时的Rust编译拒绝，以及新workspace最初影响原probe成员关系的metadata失败；没有声称12个运行期测试失败，也不声称前后测试源完全相同。实现后正式12项与旧probe117项通过。较早160745仅覆盖基础实现，最终运行另包含workspace排除与旧探针回归，没有补写旧清单。

`in_v1_payload_allowlist`只表示已批准的载荷范围，不能当作当前codec实现能力。已知但未支持的105/106/121可辨认，当前心跳只属于外层帧合同。普通string16零值与HeadMap -1/0在原语中分开，具体消息的Java-null语义仍须由后续DTO/codec实现并验证。

本轮新代码从固定Java码表/字段合同实现，原测试探针代码仍独立保留，没有使用RSeata实现或把它改名充作正式组件。没有启动TC/数据库或下载/安装工具；第三方依赖仅从已有本地缓存复制到D盘，Cargo.lock及依赖校验和已归档。

消息DTO/codec、完整帧/HeadMap/聚合/空心跳、全部三方认证仍未完成，因此不能标记WP-01整体完成或发布完整V1。连接、TM/RM、资源恢复和后续Phase仍按原要求推进；外部TC风险未接受，完整WP00-R和共享锁延期仍开放。
