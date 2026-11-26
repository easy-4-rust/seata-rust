# WP05 未终结一阶段见证的 Undo 正文校验

沿用 [提交前仲裁契约](../../mysql-commit-preconditions-contract.md)和[上一轮 Undo 行观察](../mysql-witness-undo-observation/README.md)。原观察端会读取完整 Normal Undo 行与分片，但未解码正文；实际 MySQL 中将 `rollback_info` 改为 `not-json`，仍可返回一阶段已提交。当前修复在二阶段未终结且见证为 1 时，使用固定 Jackson 解析器检查正文，要求原 XID/BranchId 与真实回执一致、类型日志非空。无效正文返回 `UndoCodec` 错误，身份不符返回恢复冲突；二阶段终结后 Undo 被原回调合法删除时仍保留历史见证。

测试在自有 MySQL 8.4.13 的已提交业务、Undo、见证样本上，先替换为无效 JSON，再将有效正文的 `branchId` 改为 43；恢复原正文后继续检查错误锁键、非法见证值和真实回调清理。`1.99-red.log` 是未解码实现的目标行为失败，`1.99-green.log` 是修复后单例通过。双工具链完整提交前仲裁回归与 Clippy 见版本日志。运行容器身份及清理记录见 `runtime.txt`、`cleanup.log`。

正文解码不等于把 Undo 重新绑定当前物理表元数据，也不逐行比对实际业务表。公开有变化事务 COMMIT、提交确认丢失、旧尝试围栏、原 Java TC 全排他覆盖和 WP05/WP06 仍未完成。
