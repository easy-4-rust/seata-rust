# SeaORM MySQL AT 适配

对应原架构 3.4、5.3 的独立 SeaORM 适配器要求，延续 WP05 事实源。正式新增 `seata-orm-seaorm` crate，不将 ORM 依赖放进事务核心。首个接口接收真实 SeaORM QueryTrait/Statement 构建的 MySQL DML，保持原 SQL 与有序原类型绑定，并在调用方已拥有的 MySqlLocalTransaction 上执行。准备、分支登记、同事务 Undo/SHA、提交授权、未知结果、回调和恢复继续走正式 MySQL 路径；适配器不另外开启连接或提交事务。

固定 SeaORM 1.1.19（发布清单 rust-version 1.81），关闭默认驱动/日志打印，使用实体宏及 JSON、BigDecimal、chrono 值支持。Rust 1.99 和 1.85 均须实际编译与运行，不以声明 MSRV 代替验证。兼容 1.1 的实体 DML 是本增量输入基线，SeaORM 2.x、原生 ConnectionTrait/完整 ORM 读写、迁移及生成 ID 的 ORM 返回接口仍属完整适配后续要求，不因此删除原路线图范围。

SQL 仅允许正式 AT 识别器及真实表校验已支持的 INSERT/UPDATE/DELETE。拒绝非 MySQL Statement、DDL/事务控制/未支持 SQL、参数数量或预算不符及不能无损映射的值。保留 i64/u64、字节、文本、NULL、精确十进制、日期、TIME、DATETIME 与 UTC 时间点；JSON 文本 null 与 SQL NULL 区分，不用浮点中转 DECIMAL，不静默丢弃时间的亚微秒精度，不为巨大十进制指数分配无界展开字符串。Debug/错误不能暴露 SQL 字面值或绑定内容。

验收包括真实 SeaORM Entity 的 INSERT/UPDATE/DELETE 构建、有序绑定及类型/边界拒绝测试，随后以未修改原 Java TC、真实 MySQL、双 Rust 的正式阶段一提交/补偿验证独立业务、Undo/checksum/历史见证、原登记/回调及原 Java parser 读取。未通过完整 ORM/生产门禁前，不把该适配增量表述为完整 SeaORM 生态已交付。
