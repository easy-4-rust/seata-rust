本目录验证固定 SQLx 0.8.6、rustls 与 MySQL 8.4.13 的选定服务端 TLS 配置。

新增 MySqlOptions::with_tls_ca_pem 接受 1..65536 字节的 PEM CA 列表，配置阶段解析证书块，连接阶段验证 DER、证书链、有效期及原主机身份。该 CA 列表补充 SQLx 内置 WebPKI 根。选项与显式关闭回环测试 TLS 双向互斥，Debug 不显示证书正文。

red.log 是缺少公开方法的编译 RED。pem-red.log 是空证书列表未拒绝的行为 RED。options_contract 还核对空值、最大容量、超限、无效 base64、Debug 与两种配置顺序。

live_tls 在只允许加密 TCP 的自有 MySQL 上验证正确 CA 成功、缺 CA/错误 CA/错误主机名失败、显式明文失败。证书 SAN 只有 IP 127.0.0.1。独立 Required 模式连接证明 localhost 可达同一实例，避免将 DNS 或 IPv6 连接失败误作身份校验。正向数据源执行 begin、元数据读取、UPDATE 和 rollback；元数据读取会建立使用原连接配置的辅助会话。独立查询确认原业务值。测试日志及独立 MySQL 客户端均记录实际 TLS cipher，后者还记录 TLS 1.3、require_secure_transport=1 和缓存 SHA-2 密码插件。

Required 与 HostRewrite 两项削弱控制分别在预期断言失败，证明信任链绕过和将原主机名改写成证书 IP 的问题会被现场用例捕获。控制脚本会核对并精确恢复原源码。VerifyCa 控制仍通过，属于未改变目标行为的无效控制，不计入成功证据。固定驱动的 NoHostnameTlsVerifier 与当前 rustls 的错误变体有关，本目录不改动依赖实现。

首次证书生成使用系统 OpenSSL 配置产生重复 Basic Constraints，OpenSSL verify 拒绝后改用显式最小配置，保留 setup 日志。CA 与服务器证书只用于本轮自有夹具，测试私钥留在 D 盘临时目录，不归档到仓库。证书有效期两天，重现时需要在新的 D 盘目录重新生成证书并相应设置 CA 环境路径。

最低工具链的首次全量链接因 D 盘空间耗尽返回 LNK1108。核对绝对路径后，仅删除本任务 MSRV 构建缓存目录中两天以前的 30 份 PDB 调试符号，释放约 3 GiB。删除清单及首次失败日志已保留，源码、数据库数据、可执行程序和验收归档未删除。最终结果以重跑的版本工作区日志为准。

本轮属于选定服务端 TLS 配置验收。客户端证书、生产认证、其他证书链、轮换/吊销及其他网络部署配置继续按原计划验证，完整 V1 和公开一阶段提交门槛保持。
