# RM 回调本地响应写入事件

对应 [WP07 观测契约](../../wp07-observability-contract.md)，[机器总账](../../wp07-rm-callback-write-events.json)列出全部当前现场及可信 catalog 摘要。

正式 CallbackHandler 在消费原 PoolInbound 回包能力时建立一次事件作用域，以原 signed request_id、connection_generation、response_count 和固定 outcome 记录一个外层信封的结算。local_written 只说明完整本地写入；not_sent 与 unknown 直接保留原 CallError 的写入边界，cancelled 不推断网络状态。没有 TC 接收确认的 V1 响应不输出“已送达”。分支决策事件增加原代际，可与信封事件关联。资源原文、头、正文、原始错误、应用数据和凭据不进入写入事件。

原目标先在旧实现缺失三个写入事件时失败。正式实现后的真实 TCP 测试覆盖重复/反向回调一次业务效果、平面重复批次两个决策但一次写入、原代际关闭后 not_sent 且不迁移到已登记的替换连接、无效批次零决策/写入。专用作用域测试核对原结果引用保持不变、写后未知的类型分类、已轮询作用域取消及 panic 解栈单次结算。此测试中的 UnknownOutcome 是类型化输入，不宣称本轮已注入真实 socket 写超时。

双 Rust 1.99/1.85 各执行固定原 Java TC 的普通提交、提交回包丢失及回滚回包丢失三场景。每个丢回包场景原代理记录第一响应 delivery_count=0，TC 在新连接重试；两次 local_written 均与各自原始帧及不同运行时代际对应，业务/Undo/持久分支效果仍由原驱动独立验证。每版本九项削弱（误称送达、泄露字段、改 ID/摘要、混淆代际、重复/缺事件、伪造送达及损坏原帧）全部拒绝。

移除正式 CallbackHandler 的结果结算，在两工具链目标均产生 cancelled vs local_written 断言失败；恢复原字节后同目标通过。两版本 RM 组件各 77 passed / 1 ignored，全工作区全功能各 732 passed / 323 ignored，RM/MySQL 全目标全功能严格 Clippy 和格式检查通过。ignored 不是全部真实运行证明，本次原 TC 场景由专用驱动单独执行。原 TC 进程和自有数据库容器退出已核对，D 盘数据库现场保留。

该结果属于程序化复核，独立外部审查仍 OPEN。生产 subscriber/采集、其余传输/持久分支/MySQL 事件及生产安全部署仍需完成，WP05–07 和整体计划不因此关闭。

重放原证据：

```powershell
python -B tools/formal-source-junit/validate_rm_callback_write_events.py --evidence docs/phase-2/evidence/rm-callback-write-events/java_20261127_050607_56c0d181 --catalog-sha256 fba66491de2228344c62232f4b8025f52de4781e0753f639285a77752f0552fd
```
