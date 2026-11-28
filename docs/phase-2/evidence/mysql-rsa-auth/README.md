# MySQL RSA 认证依赖处置与实际认证

对应 [RSA 处置契约](../../mysql-rsa-auth-replacement-contract.md)。根 Cargo 显式使用 `third_party/sqlx_mysql` 维护副本，保持原 0.8.6 实现和许可；只有活动 Cargo.toml 和认证函数发生修改，原包压缩文件、原 Git 来源及逐文件哈希、独立补丁均保存于副本。原安装 registry 源码没有修改，也不将本地补丁称为已发布上游修复。

当前权威包：[20261128_160204_a233fa21](20261128_160204_a233fa21/result.json)，最终 catalog SHA-256：`bb89d18f9b8075c28cfb01a9e8e190da08a1a2a50e10861a90ed25b9d239bf06`。Rust 1.99/1.85、未修改原 Java TC JAR、固定 MySQL 8.4.13 和独立记录器全部封存。两版各覆盖 2048/3072/4096 位公钥的缓存 SHA-2、SHA-256 和校验 CA 的 TLS，共十八笔正式 AT 全局事务；另有八项真实错误密码、坏 PEM、1024 位弱键和错误 CA 拒绝。

每个原生用例都先实际清空 MySQL 认证缓存。原始非 TLS 报文核验真实服务器公钥、协议序号、与密钥长度对应的 RSA 密文、服务器实际 OK/ERR；TLS 从原 SSLRequest 进入透明字节转发，没有解密或协议改写。TLS 会话在持有原业务事务连接时由独立 `performance_schema` 查询核对，同时确认外部仍看到原业务值、尚无分支/Undo。弱键/坏公钥的预期故障只改写明确记录的服务器公钥；失败不发密文替代或明文回退，真实 general_log 计数与 TC/业务状态证明没有业务 SQL。

十八笔阶段一独立核验原资源/锁、一次登记和报告、实际业务与同事务 Undo/SHA-256/见证。原 TC 的提交或回滚、同连接原 ID 回包、最终业务及正常 Undo 清理/恢复历史全部核验；原 Java parser 重读十八份日志并核对完整字段。自有 MySQL 在三种认证密钥之间按原数据目录停止、重启，客户端均退出后才切换密钥。服务端私钥和 TLS 私钥仅在自有 D 盘运行目录，未收入证据或 Git。

两版全工作区各 **840 passed / 415 ignored**；最终 MySQL/SeaORM 严格 all-target/all-features Clippy 均通过。忽略项不计通过。新 provider 为 aws-lc-rs 1.19.0，实际正常图与 lock 均不再包含 `rsa`。Windows 非 FIPS 采用上游预生成 NASM 对象和已有 C/C++ 编译器；保持 Rust 1.85，未安装新 CLI，未手写 RSA/OAEP，未改原 TLS 验证要求。

使用相同 RustSec DB `c5279d0b0c9e8b70df666eb6d1edbabcfd1176bd` 前后对照：旧 `rsa 0.9.10` 的 [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html) 已不报告。全 lock 审计仍退出 **1**，保留未激活的 [time](https://rustsec.org/advisories/RUSTSEC-2026-0009.html)、[SQLite](https://rustsec.org/advisories/RUSTSEC-2026-0459.html) 两项，没有 ignore 或删除公告。这不是完整 WP07 安全/生产发布门禁通过。

兼容性边界：回环非 TLS 的认证密钥低于 2048 位现在显式拒绝，保留原 4096 位上限；生产数据源的校验证书 TLS 路径保持原语义，不宣称 FIPS。**修复当前依赖于应用根 Cargo 的 patch**；仅发布/依赖某个子 crate 不会自动继承此 patch。发布前仍需可发布维护 fork 或合入上游及消费者集成方案，不能称 crates.io 用户已自动取得此修复。

首轮 [20261128_155215_74467ddc](20261128_155215_74467ddc/result.json) 的十八笔功能和八项拒绝虽通过，但 TLS 会话采样发生在拥有式连接已关闭之后，独立核验拒绝空证据；测试辅助模块的未用函数也使严格 Clippy 失败。原日志保留，状态为 incomplete_evidence。修订轮把采样移到真实未提交业务连接，移除不需要的共享辅助模块，再完整重跑全部原生场景和双版本工作区/Clippy，通过最终独立核验。

```powershell
python -B tools/formal-source-junit/native_mysql_rsa_auth.py
python -B tools/formal-source-junit/validate_mysql_rsa_auth.py docs/phase-2/evidence/mysql-rsa-auth/20261128_160204_a233fa21
```

自有回环 TC 关闭应用认证，数据库使用一次性夹具账号和权限，不能替代生产账号、证书运维或发布认证。完整 ORM、SQL 与其余 Phase 0–7 要求继续开放。
