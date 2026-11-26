# WP05：两个独立恢复进程竞争 007 报告门闩

2026-11-25 在自有 Docker Desktop 的 `mysql:8` 镜像上运行，镜像 digest 为 `sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902`，容器内 MySQL 为 8.4.13。容器仅绑定 `127.0.0.1:43318`，使用随机独立的 `undo_race_*` 库和密码；测试后已删除。Undo schema 使用 `D:\Temp\seata-owned-undo-schema.sql`，SHA-256 `8DCD994E28D31D494B0D3F90663C74D788FDB7250EF6CE0B8267B66170E91CE9`，与仓库中的 [`undo-schema.sql`](../mysql-undo-current-constraints/undo-schema.sql) 一致。未保存密码。

执行命令：

```text
cargo +1.99.0 test -p seata-db-mysql --features experimental_changed_commit --test live_commit_preconditions distinct_processes_compete_for_one_recovered_report --offline --locked -- --ignored --exact --nocapture --test-threads=1
cargo +1.85.0 test -p seata-db-mysql --features experimental_changed_commit --test live_commit_preconditions distinct_processes_compete_for_one_recovered_report --offline --locked -- --ignored --exact --nocapture --test-threads=1
```

[Rust 1.99 最终代码原始输出](rust-1.99.0-final.log)和[Rust 1.85 最终代码原始输出](rust-1.85.0-final.log)各显示 1/1 通过；[1.99 前一轮](rust-1.99.log)与[1.85 前一轮](rust-1.85.log)也通过。每轮父测试启动两个独立测试子进程，经同一 TCP 屏障同时释放；一方输出 `REPORT_RACE=sent`，另一方输出 `REPORT_RACE=already_started`。受控 RM 对端读取一条 `PhaseOneDone` 报告并回成功响应；父测试还核对原业务连接没有报告、两条已登记连接没有第二条业务帧、007 行 `report_started=1`、业务行和 Normal Undo 各一条。

本轮修复了测试对端的竞态：败者也可先建立 RM 登记连接，随后因数据库门闩已占用而断开；测试现以实际业务报告帧辨认胜者。最终代码的双工具链目标严格 Clippy 和格式检查通过。此用例使用受控 RM 对端，未覆盖固定原 Java TC 的双进程竞争、TM 进程重建或外部写入器锁安全，WP05/WP06 整体门禁仍开放。

后续另在[固定原 Java TC 联试](../java-tc-changed-commit-report/README.md)完成了双恢复进程竞争；本目录仍只记录受控 RM 对端的独立证据。
