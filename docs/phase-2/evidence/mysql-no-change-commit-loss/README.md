# 无变化 COMMIT 确认丢失现场验证

规格事实源为 [WP05 一阶段提交契约](../../mysql-first-phase-commit-contract.md)。本轮只验证无变化、无登记分支的 `commit_no_change`；有变化业务、Undo、TC 报告及完整锁安全仍未验收。

固定自有 MySQL 8.4.13 及镜像、容器 ID、标签记录于 `container-identity.txt`、`mysql-version.tsv`。Windows 43318 中继到 WSL 43317；[故障代理](../../../../tools/formal-first-phase-commit/mysql_commit_response_proxy.py) 在 43320 逐帧转发未加密的自有 MySQL 连接，仅在 D 盘标记显式启用后截留 COM_QUERY `COMMIT` 的服务端 OK 包。它先读到真实服务端 OK，再写 `server-ok.txt` 并延迟回包。代理只用于隔离测试，不能用于生产 TLS 流量。

TDD 的运行行为 RED：原实现无 COMMIT 期限，服务端已返回 OK 后，外层 1.5 秒看门狗触发 `Elapsed(())`；该次工具输出保留于任务记录。加入原操作期限后，Rust 1.99/1.85 的 `1.99-live.log`、`1.85-live.log` 均证明服务端 OK 被截留、MySQL `general_log` 的 COMMIT 增量为 1、调用返回 `DbError::Deadline { operation: "no-change COMMIT" }` 所属的未知结果。`1.99-server-ok.txt`、`1.85-server-ok.txt` 保留代理从真实服务端接收成功包后的标记。首次 GREEN 的断言因 Windows 文本换行从 LF 转为 CRLF 而失败，随后仅将文本断言改为忽略行尾换行；该失败未作为业务失败计数。

两工具链的正常无变化提交、已登记分支拒绝、断开原连接分类各通过 4 项受影响现场回归。严格全工作区 Clippy 在 `1.99-clippy.log`、`1.85-clippy.log` 通过，`fmt.log` 为空表示格式检查通过。测试只证明无变化路径的有界未知分类；服务端 OK 丢失不能安全推断为明确失败。未验证有变化分支的一阶段原子提交、旧尝试围栏、正式 Java TC 报告或外部客户端排他覆盖。
