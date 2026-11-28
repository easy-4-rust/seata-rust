# SQLx MySQL 0.8.6 认证维护补丁

此目录是明确的本地维护副本，根 Cargo 使用 `[patch.crates-io]` 选择它；不是上游已发布修复。原 crate 压缩包 SHA-256、原 Git 来源及所有原文件哈希在 `upstream-manifest.json`。原 MIT/Apache 许可和原实现保留；vendored 原代码不作为 Java 对象迁移成果计数。

仅修改活动 `Cargo.toml` 和 `src/connection/auth.rs`：以 aws-lc-rs 1.19.0 的受维护 OAEP/SHA-1/MGF1-SHA1 接口替换旧 rsa/rand 认证依赖，使用 pem-rfc7468 解析 SPKI 公钥；保持 nonce XOR、NUL、TLS 分支、原协议和原 4096 位上限。新后端明确拒绝不足 2048 位的键，不回退到旧 RSA 或明文。独立补丁文件为 `auth-aws-lc.patch`。

Rust 1.85 基线保持不变。Windows 非 FIPS 使用上游预生成 NASM 对象；需已有原生 C/C++ 编译器，不静默安装工具，不宣称 FIPS。构建后的实际认证、错误输入、TLS 与正式 AT 验收要求见 [契约](../../docs/phase-2/mysql-rsa-auth-replacement-contract.md)。
