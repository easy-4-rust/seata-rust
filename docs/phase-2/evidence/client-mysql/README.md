# 统一客户端入口现场

权威现场：[20261128_210014_079e0c0e](20261128_210014_079e0c0e/result.json)，对应[统一入口契约](../../../phase-1/client-facade-contract.md)及架构计划 §5.3/WP07。

正式 `seata-client` 使用同一端点、身份、预算及显式传输策略拥有 TM/RM；支持 TM-only、RM-only 和双角色。先启动 RM，TM 失败则等待 RM 清理；原初始化与清理错误分别保留。关闭时两方均尝试，取消或 Drop 保留原日志语义，公开错误诊断脱敏。

双 Rust 1.99/1.85 下，原 Java TC 与真实 MySQL 完成 rbatis 原生 CRUD 的十二笔提交/补偿。原 Java Undo parser、原子 SHA-256、锁、回调和业务原值独立验证通过，四组运行均记录原 TM/RM 池关闭。默认正常依赖图没有数据库或 ORM。

实际 TCP 覆盖三种角色、相同登记字段和 socket 关闭；实际 mTLS 覆盖两个角色的相同客户端证书与错误 SAN 拒绝。真实文件租约验证关闭预算耗尽、关闭被取消、直接 Drop 后的 Pending 重开；TM 启动失败与 RM 清理同时失败不丢失任何一侧原因。没有将本地 build 成功解释为 TC ready。

两个版本各 **870 passed / 421 ignored**，新入口严格 all-target/all-feature Clippy 与默认功能测试通过。隔离源码反例在两个版本均使目标测试失败：TM 错误导致提前返回，以及跳过 RM 日志排空。每个版本/反例使用独立 Cargo 目标目录并保留原锁定版本，正式运行源码没有随反例改变。早期 Rust 1.99 大错误值检查的失败日志与源码保留于 `red-executions`。

最终 1836 个成员的 catalog SHA-256：`11438b94291d8fc3ddc6e406d9d5e0b9135bfe212fa1d720481733a2631aa22c`。源码起止 SHA 相同，独立原生验证器及回归验证器均须通过。

复验：`python -B tools/formal-source-junit/client_regression.py docs/phase-2/evidence/client-mysql/20261128_210014_079e0c0e --verify-only`。原生重跑使用 `native_client_mysql.py`；其后运行 `client_regression.py <packet>`。

仅关闭统一入口这一实施缺口；生产部署、证书运维、长时负载、全部写入器/SQL、WP05/WP07 总门禁及 Phase 5–7 均继续开放。有变化 COMMIT 仍为显式实验性功能。
