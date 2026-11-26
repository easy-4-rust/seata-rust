# 持久 TM 来源绑定 TC 传输信任

对应规格：[持久 Launcher 设计](../../tm-durable-launcher-design.md)，机器总账：[wp07-tm-transport-binding.json](../../wp07-tm-transport-binding.json)。本项验证的是显式 TLS profile 下的持久来源保护，WP07 与整体计划保持未完成。

## 问题与实现

真实 RED 证明：原 TLS Begin 持久化后，原管理器关闭，同端点明文替代者完成注册，并向旧快照返回伪造 Committed。正式修复将 TLS 1.3/mTLS profile、规范化 SAN、排序去重的显式信任根集合绑定为独立版本域 SHA-256。摘要不含客户端身份或服务端叶证书，不授予查询、接管或结束权限。

迁移 014 保存新 TLS 来源为版本 2 与非空 BINARY(32)；旧明文保持版本 1/NULL，未执行 014 的旧明文 schema 也兼容。TLS 在缺少 014 时保留已创建 XID 并返回原持久化错误，不交付业务 Launcher、不自动结束事务。旧运行时只支持版本 1，会拒绝版本 2。部署管理员在 014 后执行 tm_launcher_transport_binding_upgrade_grants.sql；仅增加写角色 INSERT 新列，不授予任一运行角色 UPDATE 绑定。正式列权限属于信任边界。

查询前拒绝不同信任配置，TLS 与明文不能互换；来源版本 2 的列缺失、NULL、非 32 字节均拒绝。原拥有者更新、持锁接管及恢复发送 CAS 同时核对版本和绑定，仍需真实旧进程停止证明、实例权限、期限、代际及原不可重发门闩。

## 实际验证

- Rust 1.99/1.85 各五组真实 TLS/MySQL 用例：明文替代零 Status；真实受监控子进程退出后，持锁改写绑定使 CAS 拒绝、恢复原字节后同来源成功接管并结束；旧 schema 兼容与 TLS 明确 1054；元数据损坏拒绝后原 handle 继续完成；真实角色列权限。
- 固定原 Java TC cba73e180fd8896a99df6415b859d275122feaa7，双版本各 Commit/Rollback 丢回包：原成功结束响应实际丢弃，仅一次结束帧，同信任观察者查询原 TC；扩大根集合的管理器完成注册但零 Status。原状态含 AsyncCommitting 等，不能全部解释为全局终态。
- 三种正式源代码变异在每版本均导致目标行为断言失败：关闭查询信任门禁、移除持锁 CAS 绑定谓词、允许版本 2 缺列回退。恢复原字节后目标通过。
- 只读校验器独立计算公共 DER 信任摘要、解析原 V1 帧与 TLS 字节计数，每版本拒绝 12 项 native 证据削弱，每原 Java profile 拒绝 12 项。程序校验不等于独立人工审查。
- 两版本全工作区全功能各 743 passed / 330 ignored；四个受影响组件全目标严格 Clippy、默认配置测试/检查与默认无 rustls 依赖检查通过。忽略项中的受选 live 用例已由上述现场驱动单独执行。

这是同主机 TLS 终止代理到原 TC 的 loopback profile，不声明 Java TC 原生 TLS 或生产认证部署完成。测试私钥仅在 D 盘，归档只有公共 DER；自有进程及 MySQL 容器已清理。根/SAN 受控迁移、生产证书运维、独立外部审查和剩余 WP05–07/Phase4–7 开放。

## 权威现场

- red: [red_20261127_055852_275cc11d](red_20261127_055852_275cc11d/catalog.json)，catalog SHA-256 `415906b046b6d55144f0348091ddf2c2f2febe65e991cbbb9c4e2b75c6554975`。
- native_199: [run_20261127_064055_6e342ede](run_20261127_064055_6e342ede/catalog.json)，catalog SHA-256 `ab842a99bb93a3521f0aea35b6aaa1cccab31752e78323736a21eaa5edd503bc`。
- native_185: [run_20261127_063314_c1b0d89f](run_20261127_063314_c1b0d89f/catalog.json)，catalog SHA-256 `2d3804ea0eaa893d6d87eb9190b4358a76519194aebf829588e4d13d4820fc59`。
- original_java_tc: [java_20261127_063304_d8a8e01f](java_20261127_063304_d8a8e01f/catalog.json)，catalog SHA-256 `2a2c15b95ffda2e63c85c9b824a57e9fdb6e0e795583dee5ecd9362b22035fb8`。
- source_mutations: [mutations_20261127_063754_cdf54159](mutations_20261127_063754_cdf54159/catalog.json)，catalog SHA-256 `367e10c5a7affe6aaf9c77345033b35e10914a7cee96e1c16b1522c8a80e1e5e`。
- raw_validation: [validation_20261127_064725_f9cda5b1](validation_20261127_064725_f9cda5b1/catalog.json)，catalog SHA-256 `b35394aaad28d367c76a7bb001f9f0533bb2947d71a1f14d429be14ac3898f17`。
- default: [default_20261127_064726_19771c26](default_20261127_064726_19771c26/catalog.json)，catalog SHA-256 `b7034e72a946f64633913e5bb63d74dd047382ba68da5348899db00f1c3c3722`。
- regression_199: [regression_1.99_20261127_064105_549fe87b](regression_1.99_20261127_064105_549fe87b/catalog.json)，catalog SHA-256 `074ff40a3421c4d6dba7a95e78a4da3f9fbeba099c16726530e886c9189cec9e`。
- regression_185: [regression_1.85_20261127_064105_df312d62](regression_1.85_20261127_064105_df312d62/catalog.json)，catalog SHA-256 `f0f99474f91b3483a2003172a526f328bb02eb0a84a3743c575177c6c117af05`。

其余 pilot/中断/编译或观察器诊断保留原字节，不计入通过数。060218 现场因夹具等待已确认归属的子进程而中断，记录于其 fixture-interruption.json，后续显式注册 Ready 屏障修复；没有放宽正式超时。早期 mutation result 的状态字符串沿用 native 标签，应以 expected_failure、live 退出 101、目标断言和父 mutation 总账为准。

可复核命令见机器总账中的路径/摘要，将 native_199、native_185、original_java_tc、source_mutations 四组传给 tools/formal-source-junit/validate_tm_transport_binding.py 的相应目录与 SHA 参数；校验器不会修改原现场。全量回归及默认检查的原命令与退出码保存在各目录 commands.json。
