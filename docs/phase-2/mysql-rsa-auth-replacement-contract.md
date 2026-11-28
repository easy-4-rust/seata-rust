# MySQL 认证 RSA 依赖处置

延续 WP07 依赖审计与 Rust 1.99/1.85 基线。当前 `sqlx-mysql 0.8.6` 的 `rsa 0.9.10` 位于实际正常依赖链，RustSec RUSTSEC-2023-0071 继续报告。修复版 rsa 0.10.0 的发布清单要求 Rust 1.87，不能以提升项目 MSRV 或忽略公告替代处置。

在独立 `third_party/sqlx_mysql` 保存原 0.8.6 源码、许可和文件哈希，通过根 Cargo patch 使用明确的本地维护补丁。仅替换认证中的 PEM/SPKI 公钥解析与 OAEP 加密依赖，移除旧 rsa/rand 依赖，使用已发布的 aws-lc-rs 1.19.0（声明 Rust 1.71）和 pem-rfc7468。OAEP 的 SHA-1/MGF1-SHA1、nonce XOR、密码末尾 NUL、协议序号/包体与 TLS 分支保持原 MySQL 语义；不自行编写 RSA/OAEP，不引入私钥处理或明文认证回退，不修改已安装 registry 源码。

AWS-LC 的公钥加密接口拒绝不足 2048 位的密钥，并继续保留原 rsa::RsaPublicKey::MAX_SIZE 的 4096 位上限。原生产数据源要求校验证书的 TLS，RSA 公钥请求仅在明确允许的回环非 TLS 夹具出现。本增量验收 2048/3072/4096 位原 MySQL 公钥及无效/弱键拒绝，未认证弱键不补做不安全回退；TLS 配置与身份验证要求不降低。非 FIPS Windows 构建使用上游 prebuilt-nasm feature，无需额外安装工具；不将此路径标为 FIPS 认证。

必须以双 Rust 实际构建、原 MySQL 冷缓存 caching_sha2_password 完整认证及真实协议帧证明新加密执行成功；错误密码、坏公钥须拒绝且不执行业务。补测 TLS 认证及正式本地业务/Undo/恢复烟测，避免只靠编译或加密 round-trip。原始公告与依赖树须证明 rsa 不再出现在锁文件/实际图中，不添加 ignore；其余 time/SQLite 公告和完整安全/生产门禁分别保留。

来源：[rsa 0.10 清单](https://raw.githubusercontent.com/RustCrypto/RSA/v0.10.0/Cargo.toml)、[SQLx 0.8.6 原认证依赖](https://raw.githubusercontent.com/launchbadge/sqlx/v0.8.6/sqlx-mysql/Cargo.toml)、[AWS-LC OAEP](https://docs.rs/aws-lc-rs/1.19.0/aws_lc_rs/rsa/struct.OaepPublicEncryptingKey.html)、[Windows 构建要求](https://aws.github.io/aws-lc-rs/requirements/windows.html)。
