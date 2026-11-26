# 显式 Java V1 Getty 帧候选验证

本工具在固定 Go 提交的全新副本按顺序应用 0001–0004，不修改原始参考源码、JAR、默认 registry 或全局单例。它验证实际 `RpcPackageHandler.Read/Write`；完整 Go SDK 的配置选择、重连和业务分发仍需后续接入。

```powershell
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/go-java-v1-getty-profile/run.py
python tools/go-java-v1-getty-profile/test_validation.py
```

运行需已有 D:/E: 固定 Java/Go 构建环境，不安装依赖、不切换分支。测试数据、独立 TC 文件存储与输出均位于 D:/E:；28091 必须空闲，结束后只清理自己启动的 TC 进程。

继承新鲜 Java475、Rust117 与 Go 载荷回归后，执行实际 Getty/message 包基线和七项新增测试：326帧、每帧全部前缀及连续两帧、空/可空/有序头、非法长度/UTF-8/重复字段、原始错误原因、有符号请求ID、精确8MiB帧及32767字节头边界、默认路径保留。

326帧明确分为两组：9帧由固定 Java 实际编码；317帧由工具将新鲜已认证的29普通、38合并、250异常载荷封装。第二组不计作额外Java帧编码或额外Java测试数。

真实 TC 探针显式注入同一 handler，通过 TCP 三段写入注册和请求。使用原 Go 的1.1.0版本使原 Java TC 选择60合并响应，验证 Begin/Status/Begin 的异构结果顺序、两个原始 XID 的 Begin 查询、回滚和 Finished 查询，共8请求/8响应、不重发事务。TC 使用 FILE、关闭认证，不包含 RM、真实 Undo 或故障恢复能力。

`HeaderEntries` 记录原始顺序和 nullable value；`HeadMap` 是非 null 项视图。编码可仅提供前者，或仅提供后者（按 key 排序），同时提供时必须一致。默认入口遇到 `HeaderEntries` 明确报错，避免丢失语义。

`prepare_candidate.py` 仅用于主动更新可审查补丁，不会在验证时自动刷新。验证同时核对补丁/清单哈希、全部714原始成员、继承候选、新候选、实际测试与工具闭包，并比较格式化后的可编辑模板与实际执行文件，防止旧补丁认证新源码。

规格：[Getty 帧阶段计划](../../docs/phase-0/go-java-v1-getty-profile-plan.md)。证据：`docs/phase-0/evidence/local-go-java-getty-profile/`。本候选通过不关闭 WP-00，不等同正式 Rust 协议或生产可用交付。
