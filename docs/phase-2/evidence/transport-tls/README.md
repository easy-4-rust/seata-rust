# Seata 原生传输 TLS 1.3 / mTLS 证据

规格：[传输 TLS 契约](../../wp07-transport-tls-contract.md)。[机器总账](../../wp07-transport-tls.json)列出最终现场及明确 catalog 摘要。

正式 TcpConnection、raw/登记池及 TM/RM builder 增加显式 mTLS 入口。tls 特性默认关闭；根证书、客户端链、密钥及 DNS/IP SAN 必须显式提供并经预算/格式校验。TLS 1.3、完整链与 SAN 验证、每代际完整握手均固定启用，不提供明文回退或跳过验证。连接期限同时覆盖 TCP 与 TLS。现有 Copy 配置及默认明文接口保持兼容，原 V1 登记、回调代际、业务发送及重试边界继续由正式组件控制。

双 Rust 1.99/1.85 各八组真实 TLS 目标通过：完整双向认证、原 signed 回调/可空头、错误 CA/SAN、无信任客户端、明文对端、受阻握手期限和关闭、已登记池重连。TLS 1.3 客户端完成握手可能早于服务端拒绝其最后一轮证书，因此该后续错误仍按原帧写入标记分类，不伪称必然写前失败。一个 ignored 导出器由 Java 驱动明确执行，测试密钥只留在 D 盘；仅公开 DER 证书进入证据。

锁定 tokio-rustls 0.26.4 的 poll_write 可能先接受明文并保留待写密文，因此正式 writer 现在在原写期限内完成 write_all + flush 后才发布写入回执。容量 1 双工流和真实 BufWriter 的目标先因提前成功失败，修复后验证完整排空及 flush 写期限届满仍返回 UnknownOutcome。两版本各 27 个传输库测试通过。

固定原 Java TC 双版本各三场景通过：正常提交、提交回包丢失、回滚回包丢失。正式 TM/RM 至自有代理协商 TLS 1.3 并以原客户端证书认证，代理至原 TC 的明文后端仅限同机回环。原 TC、原报文观察器、MySQL 及 Undo 验证逻辑保持原有决策。每个 TLS 会话的上/下行字节数与解密后的原 V1 帧逐连接一致；RM 丢回包后的新连接重新完成 mTLS 和资源登记。原 TM 在自然回滚等待后可能更换连接，新会话只重新登记并查询 Status，没有重放 Begin/end。每版本九项证据削弱全部拒绝。

把正式池的 TLS 调用改为明文，使真实握手/Ready 目标在两版本失败；恢复原字节后通过。移除正式 writer 的 flush，在两版本都触发缓冲写入提前成功断言；再次恢复后通过。最终全工作区全功能各 742 passed / 324 ignored；四个受影响组件全目标严格 Clippy 通过。默认特性下四组件测试及严格 Clippy 也通过，默认 seata-transport 无 rustls 运行依赖；一次 Windows 示例链接失败保留在早期 default 现场，后续完整复测通过。

历史目录保留：052626 为代理脚本属性调用错误；052750 为测试 CA/叶证书同名导致 OpenSSL 拒绝；053012 为补充 flush 语义前的成功现场。最终验收只使用机器总账指定的 053628 现场，其他目录不替代最终源码验证。夹具通过不同 CA/叶证书名称修正链构造，未关闭验证。自有代理线程、TC/客户端/观察器进程及数据库容器退出均核对，D 盘现场保留。

这是程序化复核，独立外部审查仍 OPEN。该结果不声称 Java TC 原生 TLS、生产代理已部署或完整 V1 已完成；证书签发/轮换/吊销、生产身份授权、跨主机后端及完整安全/负载/部署门禁仍需完成。

```powershell
python -B tools/formal-source-junit/validate_transport_tls.py --evidence docs/phase-2/evidence/transport-tls/java_20261127_053628_6d3dabfb --catalog-sha256 efca67745fe991089ac9ac9494bcf641154b2c7c3ff11534a6ed558c90bbfd4f
```
