# TM Launcher 008 来源表现场证据

对应[跨进程恢复设计](../../tm-durable-launcher-design.md)的第一项数据库结构子门禁。2026-11-25 在仅绑定 `127.0.0.1:43318` 的自有 MySQL 8.4.13 容器中，每个 Rust 工具链使用独立随机 `undo_tm_*` 库；镜像固定为 `sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902`，测试后容器已删除。随机数据库密码未保存。

先在没有 CHECK 约束的 008 草案上执行目标测试，[红灯](schema-red-1.99.log)实际将 `send_started=1`、`end_operation=0` 的非法来源行写入，`unwrap_err` 因收到成功插入而失败。加入约束后，最终代码的 [Rust 1.99](schema-green-1.99.log) 与 [Rust 1.85](schema-green-1.85.log) 各通过 1/1；双版本目标严格 Clippy、格式检查通过。目标命令为：

```text
cargo +1.99.0 test -p seata-db-mysql --test live_tm_launcher_schema --offline --locked -- --ignored --exact launcher_source_schema_preserves_full_xid_and_rejects_invalid_claim_states --nocapture
cargo +1.85.0 test -p seata-db-mysql --test live_tm_launcher_schema --offline --locked -- --ignored --exact launcher_source_schema_preserves_full_xid_and_rejects_invalid_claim_states --nocapture
```

测试覆盖原生上限 32767 字节 XID、摘要唯一冲突 1062、空 XID/无意图已发送/非法操作的 CHECK 拒绝 3819，以及两条独立连接争抢原操作和发送门闩 CAS 时各仅一条影响行。摘要是否由实际 XID 正确计算、完整原文比对、源写入与真实 Begin 的因果关系、旧拥有者停止认证、结束帧发送与故障恢复尚未实现或验证。008 表此时未接入正式 TM，不构成 Launcher 恢复权限，WP05/WP06 整体门禁继续开放。

## 后续 009 兼容迁移

后续正式 TM 实现使用独立的 `009_seata_rs_tm_launcher_owner_instance.sql`，在既有 008 表上增加允许 `NULL` 的 32 字节实例 ID，不修改 008 基线。现场 `legacy_008_row_remains_unowned_after_009_migration` 先按 008 建表并写入历史行，再应用 009；Rust 1.99/1.85 各通过 1/1：历史 ID 仍为 `NULL`，带实例 ID 的 CAS 影响零行，只读快照拒绝该行。正式 Begin 写入非空随机 ID。此处旧行不被自动补发或接管；可信旧进程停止证明与生产权限仍开放。较早段落所述“尚未实现”仅指 008 结构门禁当时的状态，正式 TM 后续进展见[来源与发送门禁证据](../tm-launcher-store/README.md)。
