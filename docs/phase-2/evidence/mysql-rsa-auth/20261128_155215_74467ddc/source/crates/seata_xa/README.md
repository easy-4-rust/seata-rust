# seata-xa

Phase 4 的独立 XA 引擎身份基础。`XaBranchXid` 与固定 Java 的原 UTF-8 gtrid、`-` 分支前缀及格式号 9752 一致；null、空串、零和有符号 long 的值语义保留。字节恢复入口额外拒绝无效 UTF-8 与非规范十进制；MySQL 出站仅允许非空 64 字节内 gtrid 和正原 BranchId，并使用原字节的十六进制字面量。

这只是字节身份，不是 TC BranchRegister 回执，也不授予全局结束权。模型不生成 AT Undo。MySQL 原生两阶段实现位于 `seata-db-mysql` 的可选 `xa` 特性；RM 持久仲裁和原 Java TC 的完整 XA 门禁继续实施。规格见 [MySQL XA 契约](../../docs/phase-2/mysql-xa-contract.md)。
