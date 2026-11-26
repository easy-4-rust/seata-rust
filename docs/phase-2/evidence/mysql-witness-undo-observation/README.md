# WP05 一阶段见证与未终结 Undo 联合观察

沿用 [提交前仲裁契约](../../mysql-commit-preconditions-contract.md)。原 `first_phase_commit_observed` 在恢复行 `first_phase_committed=1` 后仅检查原回执及锁键；若二阶段尚未终结却提前丢失 Normal Undo，仍可返回 `true`。现在同一独立观察事务先锁定恢复行，再读取并校验原 Normal Undo 的行、状态、分片身份及长度；缺行返回恢复冲突。二阶段终结后原回调可合法删除 Undo，历史见证仍保持可读。

真实 MySQL 8.4.13 自有 schema `undo_commit_observe_20261124` 上，先修改现有用例：管理事务原子写入业务、Undo、见证并提交后，故意删除原 Undo，要求观察接口返回 `RecoveryConflict`；恢复原 Undo 后继续核对错误锁键、非法见证值和二阶段清理后的历史观察。旧实现失败见 `1.99-red.log`；修复后单例通过见 `1.99-green.log`。Rust 1.99 与 1.85 各串行通过 `live_commit_preconditions` 全部 17 项，分别见 `1.99-regression.log` 与 `1.85-regression.log`。全工作区严格 Clippy 见对应日志，格式检查通过。自有容器身份、版本和清理见 `runtime.txt`、`cleanup.log`。

该观察仍不逐行重读业务表，也不重新绑定 Undo 的当前物理元数据。单个 `first_phase_committed=1` 不是公开有变化事务 COMMIT 的充分证明；原 TC 排他锁完整覆盖、旧尝试围栏、提交确认丢失、报告和 WP05/WP06 仍开放。
