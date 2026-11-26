# WP05 自有 Windows MySQL 8.4.13 准备与提交现场

2026-11-24 在 D 盘自有隔离实例运行。[Oracle 官方 `mysql-8.4.13-winx64.zip`](https://dev.mysql.com/get/Downloads/MySQL-8.4/mysql-8.4.13-winx64.zip) 的下载页 MD5 为 `7b94489d5524dea9b27097068bea25a8`，下载文件实际校验一致；程序与数据目录均位于 `D:\seata-mysql-native-8.4.13`。未注册 Windows 服务，经典协议仅监听 `127.0.0.1:43318`。运行事实见 [mysql-facts.log](mysql-facts.log)：`VERSION()=8.4.13`、InnoDB、`REPEATABLE-READ`、关闭 binlog，数据库为自有 `undo_native_validation`。Undo 建表来自 `D:\codex-builds\seata-wsl-mysql-20261120\init\00-undo.sql`。原始 ZIP 校验后已删除；本目录保留执行日志和代理收到原 OK 的标记。测试后使用 `mysqladmin shutdown` 正常关闭（退出码 0），确认 43318 无监听且自有 `mysqld.exe` 进程已退出。

| 工具链 | 实际 MySQL 准备回归 | 提交前置条件回归 | 原 COMMIT OK 响应丢失 |
|---|---|---|---|
| Rust 1.99 | [15/15](prepare-1.99.log) | [26/26](commit-1.99-no-proxy.log) | [1/1](commit-loss-1.99.log)，[原 OK 标记](commit-loss-1.99.ok) |
| Rust 1.85 | [15/15](prepare-1.85.log) | [26/26](commit-1.85-no-proxy.log) | [1/1](commit-loss-1.85.log)，[原 OK 标记](commit-loss-1.85.ok) |

`live_first_phase_preparation` 用 `--ignored --test-threads=1` 串行执行；新增用例核对 Undo 冲突后的显式回滚和单次 `PhaseOne_Failed`，以及注册等待取消后调用方进度槽的未知分类。`live_commit_preconditions` 先用 `--skip experimental_changed_commit_server_ok_loss_is_unknown_with_durable_witness` 执行其余 26 项，再用 `tools/formal-first-phase-commit/mysql_commit_response_proxy.py` 独立执行该故障单例。代理将原 `COMMIT` 转发到自有 MySQL，收到服务器 OK 后写入标记并扣留响应；测试核对原回执、Unknown 分类、独立持久见证和原 MySQL general log 的单次 COMMIT。两套工具链目标的严格 Clippy、默认/实验构建和格式检查另已执行。

首次整组准备回归在夹具创建 `_other` 数据库时因已有同名自有库失败，另外 14 项通过；将夹具改成 `CREATE DATABASE IF NOT EXISTS` 后，双工具链完整 15 项复跑通过。首次无代理运行提交目标时，专用故障单例因未设置代理变量而停止，其余 26 项通过；随后按上表分组重跑并通过。不能把这些环境/夹具失败计为产品故障，也不能把分组结果称为一次 27 项无条件单进程通过。

本现场使用 Windows 原生 MySQL 和受控 Rust RM/TCP 对端，尚未验证固定原 Java TC、Linux 固定镜像、全局锁覆盖、旧尝试持久围栏、跨进程恢复或 WP05/WP06 的生产 Gate；实验有变化 COMMIT 仍默认关闭。
