# TM 停止证明双容器节点验收

运行器使用既有固定镜像提供 Linux 用户空间，直接启动 Rust 夹具。它为签发节点和接管节点生成各自的 TLS 证书、配对指纹与停止签名配置，再分别进行只读挂载。签发节点保有两把测试停止签名私钥。接管节点进程检查自己的凭证目录中只有本方 TLS 私钥和签发方的停止公钥。两个节点共享的状态目录仅用于公布已停止的 TM 实例 ID 和就绪标记，在接管节点上只读。

运行命令：

```powershell
& 'tools/formal-first-phase-commit/run_tm_stop_nodes.ps1' -ZigExe <已安装的 Zig 可执行文件路径>
```

运行机器须有稳定版 Rust 及其 Linux GNU 目标、Zig 和本地已缓存的固定镜像。运行器离线构建，以 Rust 1.99.0 和 Zig 0.16.0 通过，并记录 Linux 测试程序的 SHA-256。无宿主端口暴露，两个容器加入独占内部网络。签发节点监护独立 TM 子进程，确认退出证明后才提供签名。原公钥接受一次、轮换期与退役后新公钥共接受两次、退役旧公钥拒绝一次。服务端记录用于 TLS 的两个非回环地址，双方节点退出码均为 0。[运行结果](run-f81e601e34c5/verified.json)，[清理记录](run-f81e601e34c5/cleanup.json)。

Rust 1.85 的本机双向 TLS 测试和 Rust 1.99 / 1.85 的目标严格 Clippy、格式检查通过。该现场验证一台 Docker 主机上的两个隔离容器，真实 MySQL 代际更新与 AT 终态仍由既有联试独立验证。跨管理域配置分发、独立主机和 MySQL 接管的联合验收尚未完成。

## 远程签发与原 Java TC 的联合 AT 联试

增设 `run_tm_attested_java.ps1 -ZigExe <已安装的 Zig> -Scenario tm_attested_at_commit` 和 `tm_attested_at_rollback_loss`。签发容器运行 Linux Rust 1.99 构建物，实际监督执行一阶段的旧 TM 进程。旧 TM 提交结束意图后退出，签发容器仅在确认退出后响应双向 TLS 挑战，停止签名私钥随角色专用目录只读挂载。Windows 接管方只使用签名公钥和自己的 TLS 凭证，从真实 MySQL 读取来源并走 `transfer_attested_owner`。原 Java TC、TM/RM 原始帧观察器与独立 MySQL 查询共同核验代际和 AT 终态。

| Windows 接管方 | Commit | Rollback 回包丢失 |
|---|---|---|
| Rust 1.99 | [结果](../java-tc-changed-commit-report/run-tm_attested_at_commit-1.99-ac16e88f/verified.json) | [结果](../java-tc-changed-commit-report/run-tm_attested_at_rollback_loss-1.99-a043dbed/verified.json) |
| Rust 1.85 | [结果](../java-tc-changed-commit-report/run-tm_attested_at_commit-1.85-931f0d09/verified.json) | [结果](../java-tc-changed-commit-report/run-tm_attested_at_rollback_loss-1.85-e999d63e/verified.json) |

四组均为远程签名接管，代际为 2，发送标记为 1；原 TC 各收到一次全局结束请求并发出一次匹配的原分支回调，TC 全局、分支与锁记录最终清零。Commit 保留一行业务数据并清除 Undo。Rollback 清除业务行和 Normal Undo，保留防悬挂记录，TM 在回包丢失时保留未知并拒绝通过原内存授权恢复。证据目录含 `signer.log`、原始帧、终态 SQL 和进程清理记录。双工具链目标严格 Clippy、格式检查与 Python 语法检查通过。联合测试边界是同一 Docker Desktop 主机的 Linux 容器与 Windows 宿主；生产多主机配置分发和独立主机验收继续开放。

## 010 信任域绑定回归

同一联试使用 010 迁移复跑。旧 TM 将预配域写入来源，新 TM 在远程接管前读取并校验该值。报告要求四组均确认 `origin_stop_scope_bound=true`、`remote_stop_proof=true`，并保留全局结束、分支回调、AT 终态和进程清理的检查。

| Windows 接管方 | Commit | Rollback 回包丢失 |
|---|---|---|
| Rust 1.99 | [结果](../java-tc-changed-commit-report/run-tm_attested_at_commit-1.99-a72a8f35/verified.json) | [结果](../java-tc-changed-commit-report/run-tm_attested_at_rollback_loss-1.99-333b1e9f/verified.json) |
| Rust 1.85 | [结果](../java-tc-changed-commit-report/run-tm_attested_at_commit-1.85-3061d90f/verified.json) | [结果](../java-tc-changed-commit-report/run-tm_attested_at_rollback_loss-1.85-8bffbb0d/verified.json) |

## 严格停止签名验证

停止证明负向测试在旧验证路径中复现了异常密钥和签名被接受。接管方改用严格 Ed25519 验证并拒绝弱公钥；含无效公钥的轮换不会替换原清单。回环和双向 TLS 测试覆盖无效输入、合法签发、公钥重叠和退役。

本次加固后重跑了 Linux 签发容器与 Windows 接管的真实 AT 联试。原 Java TC 仍对原分支完成一个二阶段回调，MySQL 核对新代际和原业务终态。[Rust 1.99 Commit](../java-tc-changed-commit-report/run-tm_attested_at_commit-1.99-3835921b/verified.json) · [Rust 1.85 Rollback 回包丢失](../java-tc-changed-commit-report/run-tm_attested_at_rollback_loss-1.85-c8a98f01/verified.json)。
