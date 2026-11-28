# SeaORM 借用式原生连接

延续已有 SeaORM/WP05 规格。新增可选 borrowed_connection 特性，启用 SeaORM 原生 SQLx MySQL 行转换和 ProxyExecResult 回执构造；默认 DML 输入适配不启用 ORM 驱动。核心仍不依赖 SeaORM；不创建 ORM Pool，不暴露原 SQLx 连接、COMMIT 或数据库代理事务。借用式 SeaOrmMySqlConnection 实现原生 ConnectionTrait，独占借用已有 MySqlLocalTransaction，内部串行化调用；释放借用后才能交回正式 prepare/commit/recovery 流程。不实现 TransactionTrait，不用代理事务的空 begin/commit/rollback 冒充真实事务。

DML 继续通过原识别、类型绑定、镜像/Undo/SHA/登记边界。读取必须在同一原事务和 RR 快照上进行，读取自己的未提交写入；直接返回原生 SQLx 行，让 SeaORM 保留列顺序、索引、数值宽度和原生时间转换，不能用 BTreeMap 代理行改变索引或猜模型类型。

首个读取增量允许有界单表直接列/列别名或单独通配符、正式安全谓词、简单列排序和受限 LIMIT/OFFSET。每次预检长度再取值，最多 1024 行、128 列、单值 1MiB、总值 8MiB；原绑定顺序及空值不改变，显式分页参数在数据库调用前验证。拒绝多语句、注释/条件注释、变量赋值、函数/UDF、INTO/OUTFILE、连接、子查询/聚合、锁定读、事务控制和未支持表达式。普通查询使用原 RR 快照，不以 FOR UPDATE 当前读代替。失败/取消/超时保持原事务毒化与拥有式关闭边界，不发布部分结果或绕过 Undo。

验收包含真实 SeaORM Entity 的生成主键插入并返回完整模型、查询自己的未提交行、模型更新/删除、精确 DECIMAL/JSON/二进制/NULL、按列索引读取、独立外部连接仍不可见、拒绝副作用查询与预算超限、原借用释放后的正式提交/补偿、原 Java parser、双 Rust 工具链和生命周期/缺少 TransactionTrait 的编译验证。关联、聚合/分页完整行为、流式查询、嵌套事务、迁移和 2.x 不因本增量自动完成。
