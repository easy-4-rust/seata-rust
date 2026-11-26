# seata-db-mysql

WP05真实SQLx/MySQL基础与受限本地DML：有界事务、MDL/完整元数据、物理域、前后镜像与内存Undo准备。公开读取/执行/回滚，当前没有COMMIT入口。

[最新执行证据](../../docs/phase-2/evidence/mysql-execution-gate/README.md)与[契约](../../docs/phase-2/mysql-execution-images-contract.md)列明显式PK INSERT、UPDATE/DELETE、预算、取消/超时和失败回滚边界。INSERT内部临时键赋值需要CREATE TEMPORARY TABLES；不自动授权，不猜自增键。原[前镜像](../../docs/phase-2/evidence/mysql-row/README.md)及[查询间竞争](../../docs/phase-2/evidence/mysql-image-race-gate/README.md)保留历史执行范围。

现场MySQL8.4.13、lower_case_table_names2、显式loopback非TLS。Java Undo序列化、同事务持久化、正式分支注册、提交未知恢复及生产TLS/最小权限仍未完成，WP05/06继续开放。


[最新原生类型身份](../../docs/phase-2/evidence/mysql-type-gate/README.md)保留18种DATA_TYPE及tiny显示宽度，不转换原FieldValue。原Java对照已固定选定运行来源；Rust日志codec与落库仍在实施。
