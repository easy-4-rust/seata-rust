# 原 TC 的单向 Undo 保留期清理

正式契约：[mysql-undo-retention-contract.md](../../mysql-undo-retention-contract.md)。本增量接入已登记 RM 原连接的 `UndoLogDeleteRequest`，保留原 Java Undo 格式及已有摘要、恢复记录。

验收现场：[`20261128_143040_b6169354`](20261128_143040_b6169354/result.json)，最终 catalog SHA-256：`a586de87d753b384f682f0bbbf7fbf0e550a85dbbd5783716ec54d68e5cc7f7b`。原 Java pin、未修改 TC JAR、实际依赖、两版 Rust 可执行文件及全部 crate 输入均封存并核验。运行器使用自有回环 MySQL 8.4.13 和关闭认证的原 TC，不构成生产认证验收。

Rust 1.99/1.85 各在独立 schema 放入 6111 条原表记录。等待原 TC 固定 180 秒初始延迟后，每版删除 6107 条过期行，包含 Normal、GlobalFinished 及恰好零点边界；四条分界后记录保留，含分界后一微秒和当日记录。原 Java 的真实 `deleteUndoLogByLogCreated` 对 7、0、-1 天均返回 `[3000,3000,107]`，最终行集与 Rust 相同。原 Java 默认天数回退、Rust 规范化及 UTC 日历单元测试分别保留，不把 TC 已规范化的 7 天帧当作 Rust 接收非正天数的原生证据。

每版先用错误 JDBC 绑定确认 6111 行保持；修复绑定后实际清理。再安装原表触发器并加入过期行，原 TC 指令后目标行保持，移除触发器后继续清理。之后通过正式 TM/RM 注册并回滚一笔未提交的本地分支，原 TC 回调成功、业务值保持 10，TC 全局/分支/锁归零。原始帧独立核验全部清理指令、拒绝和后续回调使用同一物理 RM 连接，清理没有响应；正常历史围栏不因新鲜而删除。该分支用于验证通道继续工作，不冒充已提交阶段一的行补偿证据。

两版各通过 RM 83 个测试、MySQL 库 63 个测试及受影响包严格 all-target Clippy；完整 `--workspace --all-features` 各通过 831 个测试、413 个忽略。忽略项未计为通过。资源错误、未知资源、非法模式/同步消息、默认天数和取消组件测试随源码与日志封存；真实 MySQL 权限撤销、COMMIT 丢响应/部分批次取消仍需专门验收，不关闭完整 WP05/WP07 门禁。

首轮 [`20261128_142719_8d7e8f11`](20261128_142719_8d7e8f11/result.json) 没有启用显式心跳，原 TC 在定时清理前关闭空闲客户端，已停止自有进程并保留日志，状态为 incomplete。修订轮启用正式 5 秒心跳后通过；首轮不计作验收成功。

重跑：`python -B tools/formal-source-junit/undo_retention_java.py`。独立核验：`python -B tools/formal-source-junit/validate_undo_retention_java.py docs/phase-2/evidence/undo-retention/20261128_143040_b6169354`。源码、工具及运行产物不一致时核验失败。
