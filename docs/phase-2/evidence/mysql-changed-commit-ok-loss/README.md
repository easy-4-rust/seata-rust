# WP05 实验有变化 COMMIT：服务器 OK 响应丢失

本轮仅验证默认关闭的 `seata-db-mysql/experimental_changed_commit`。受控原协议对端先完成正式准备并给出真实登记回执；MySQL 帧代理随后只拦截原业务连接的 `COMMIT` 响应。代理把 COMMIT 转发给 MySQL，收到服务器原 OK 后写入 `changed-commit-loss-*-ok` 标记，再扣留 OK。测试对两个工具链均断言接口返回 `Unknown`、保留原登记回执且原因是 `changed COMMIT` 超时。独立连接读取原业务行、Normal Undo、004 恢复见证均为 1；`mysql.general_log` 中 COMMIT 增量为 1。实验接口没有报告 TC，也没有重做业务 SQL。

| 工具链 | 故障单例 | 普通现场回归 | 默认严格 Clippy | 实验严格 Clippy |
| --- | --- | --- | --- | --- |
| Rust 1.99 | `changed-commit-loss-1.99-live.log`：1/1 | `changed-commit-loss-1.99-regression.log`：21/21 | `changed-loss-1.99-default-clippy.log`：通过 | `changed-loss-1.99-lab-clippy.log`：通过 |
| Rust 1.85 | `changed-commit-loss-1.85-live.log`：1/1 | `changed-commit-loss-1.85-regression.log`：21/21 | `changed-loss-1.85-default-clippy.log`：通过 | `changed-loss-1.85-lab-clippy.log`：通过 |

`changed-commit-response-loss-red.log` 是先写测试再扩展共享夹具时得到的 E0599 编译 RED；它仅证明测试入口最初不存在，并非行为负控。初次运行使用过短的夹具前置超时，在登记阶段失败、没有发送 COMMIT；将前置超时设为 4 秒后，两次故障单例通过。两份 `*-ok` 是代理收到原 MySQL OK 后的标记。格式检查另以 `cargo fmt --all -- --check` 通过。

本次使用自有 MySQL 8.4.13、受控原协议对端及隔离代理。原 Java TC 对所有外部写入者的锁覆盖、旧尝试持久围栏、真实 TC 一阶段报告/丢失、取消与进程退出、WP05 十四项和 WP06 均未由此验收。该实验功能继续默认关闭，生产有变化 COMMIT Gate 保持开放。运行时退出状态见 `runtime.txt`。
