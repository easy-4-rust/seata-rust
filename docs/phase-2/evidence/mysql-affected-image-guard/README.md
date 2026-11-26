# 有影响行缺失 Undo 镜像的提交前保护

延续 [WP05 一阶段提交契约](../../mysql-first-phase-commit-contract.md)。`MySqlLocalTransaction::execute` 在真实执行器返回后检查：若驱动报告受影响行数大于零，而 before/after 镜像同时为空，原作用域保持中毒并返回镜像错误，不能走无变化准备或 COMMIT。零行、空镜像仍可作为合法无变化结果。该保护不替代实际 SQL 解析、完整镜像、Undo 和原 TC 锁覆盖验收。

TDD 首先在缺少检查函数时得到 E0599；两 Rust 工具链各通过一个正反形状单元测试，日志为 `1.99-unit.log`、`1.85-unit.log`。它验证拒绝规则，未模拟真实 MySQL 驱动错误返回“影响一行但零镜像”。

真实 MySQL 8.4.13 自有库执行完整 17 项 DML 现场回归，Rust 1.99/1.85 分别见 `1.99-live-execution-verified.log`、`1.85-live-execution-verified.log`；INSERT、UPDATE、DELETE 和无变化等合法路径均通过。另各执行一次原连接无变化 COMMIT，见 `1.99-no-change-commit.log`、`1.85-no-change-commit.log`。首次现场运行缺少基础测试表，17 项均失败；补入测试表后仍缺原 Java `undo_log` 表，16 项通过；补入该表后，自有 `mysql.general_log` 的上轮记录残留使一次调用计数失败。三轮日志分别为 `1.99-live-execution.log`、`1.99-live-execution-final.log`、`1.99-live-execution-final2.log`，不计为产品 RED。核对自有容器 ID/标签后清空其测试通用日志，最终整组通过。

严格全工作区 Clippy 在 `1.99-clippy.log`、`1.85-clippy.log` 通过，`fmt.log` 为空表示格式检查通过。`cleanup.log` 记录自有中继、容器和隔离引擎按身份关闭。公开有变化业务 COMMIT、旧尝试围栏、TC 报告及原 WP05/WP06 验收继续开放。
