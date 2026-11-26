# seata-db-mysql

WP05真实SQLx/MySQL基础：有界连接与本地事务、会话验证、事务内MDL及完整元数据读取、失败/取消后的poison和连接丢弃。当前仅提供读取与回滚，不提供绕过Undo的业务提交入口。

[契约](../../docs/phase-2/mysql-foundation-contract.md)和[双版本真实数据库证据](../../docs/phase-2/evidence/mysql-foundation/README.md)列明支持类型、权限和拒绝矩阵。现场为MySQL8.4.13、lower_case_table_names=2、显式loopback非TLS；生产认证、最小权限、业务行值、镜像、Undo与未知提交尚未验收。
