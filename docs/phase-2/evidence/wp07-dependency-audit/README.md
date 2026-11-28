# WP07 Rust 依赖公告审计增量（2026-11-25）

后续状态（2026-11-28）：当前根构建的 [MySQL 认证维护补丁与原生验收](../mysql-rsa-auth/README.md)已从实际图及 lock 移除旧 `rsa`，保持 Rust 1.85。新旧相同公告库对照不再报告 RUSTSEC-2023-0071，但仍保留 time/SQLite 锁项告警；发布消费者的 root patch 集成及完整安全 Gate 尚未关闭。下文是原日期的历史检查结果，不反映该后续处置已发生的状态。

范围：当前根目录 `Cargo.lock`，包含实验性 MySQL crate 的锁定依赖。此记录是 WP07 安全检查的增量，仍有未解决公告，不能作为完整安全 Gate 的通过证据。

使用 `cargo-audit 0.22.2` 和 RustSec advisory DB 快照 `10db6998b5d74ec438bcf0763125bd425a068a8f`（1567 条公告）审计。原始机读结果分别保存为 [更新前](cargo-audit-before.json) 和 [更新后](cargo-audit-after-url.json)。后一次使用同一已下载数据库的 `--no-fetch` 复扫；其 JSON 未重复填写数据库提交字段，故以第一次扫描的提交号标识这次审计的数据库快照。当前 `Cargo.lock` SHA-256 为 `4CE88A528635D7C136D8B3CD0A3D7D745C1E668207F4F750ED577BCAFD10FA6F`。

| 公告 | 更新前 | 更新后 | 处置与限制 |
|---|---|---|---|
| [RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592.html) | `url 2.5.8` | 不再报告 | 锁文件更新到 `url 2.5.9`，同时由解析器更新 `form_urlencoded 1.2.2 → 1.2.3`。`url 2.5.9` 和 `form_urlencoded 1.2.3` 声明的最低 Rust 版本分别为 1.63 和 1.51，低于本项目 1.85 基线。 |
| [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html) | `rsa 0.9.10` | **仍报告** | 依赖链为 `seata-db-mysql → sqlx 0.8.6 → sqlx-mysql 0.8.6 → rsa 0.9.10`。`sqlx-mysql 0.8.6` 把 `rsa 0.9` 声明为非可选依赖，无法仅关闭 feature 移除。没有添加审计忽略项或风险豁免。 |

`RUSTSEC-2023-0071` 描述的是 RSA **私钥解密**的计时侧信道。所检查的 `sqlx-mysql 0.8.6` `src/connection/auth.rs` 使用 `RsaPublicKey::from_public_key_pem` 和 `RsaPublicKey::encrypt` 在 MySQL 认证路径加密客户端口令；在该 crate 的源码搜索未发现 `RsaPrivateKey` 或 `decrypt`。据此推断**所追踪的 MySQL 客户端路径未触发公告所述私钥操作**，但这不是完整依赖可达性证明，也不消除审计告警。SQLx 0.9 声明 Rust 1.94 最低版本，高于项目 Rust 1.85 基线，不能直接升级来解决；后续需评估与双工具链兼容的上游修复或替代依赖，并重新审计。

更新锁文件后，Rust 1.99.0 和 1.85.0 各执行：

```text
cargo test -p seata-db-mysql --features experimental_changed_commit --lib --quiet
cargo test -p seata-db-mysql --test options_contract --quiet
cargo clippy -p seata-db-mysql --features experimental_changed_commit --all-targets -- -D warnings
```

两个工具链的库测试均为 45 通过、4 忽略；配置契约测试均为 5 通过；严格 Clippy 均退出 0。以上是离线测试，不能替代真实 MySQL 故障和竞争场景。不同进程恢复者竞争的现场测试仍受本机 MySQL 入站防火墙规则阻断，尚无有效运行结果。

WP07 安全项维持开放：完成剩余 `rsa` 公告的可验证处置，继续 SQL/Undo 边界的模糊测试、真实数据库模式和故障路径验证，并重新运行完整依赖审计。
