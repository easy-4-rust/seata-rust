# 原生借用式 SeaORM 连接与快照读取

对应 [连接契约](../../seaorm-connection-contract.md)及原架构独立 ORM 层要求。新增可选 `borrowed_connection` 特性：`SeaOrmMySqlConnection` 实现真实 `ConnectionTrait`，独占借用调用方已有的 `MySqlLocalTransaction`，串行化操作。生命周期结束后才能把原事务交回正式 prepare/commit/recovery。没有 ORM Pool、原 SQLx 连接 getter 或独立提交入口；没有实现 TransactionTrait，也没有使用代理事务的空 begin/commit/rollback。负向 rustdoc 证明不能调用独立事务接口，真实现场证明释放借用后可完成原提交/补偿。

首个 SELECT 增量具有独立密封识别器：单表直接列/别名/单独通配符、安全谓词、简单列排序及有界 LIMIT/OFFSET。拒绝函数/UDF、赋值变量、文件输出、注释、多语句、事务控制、锁定读、连接、子查询/聚合与未支持形状。最多 1024 行、128 列、单值 1MiB、总值 8MiB；先预检长度再取值，不先下载超限 BLOB。主键稳定并列行，使两次 RR 快照查询选中相同行；不把普通读取替换成 FOR UPDATE 当前读。执行前语法/绑定拒绝不访问事务，实际读取错误/超时/取消保持原 poison/rollback/close_on_drop 边界。

返回真实 SQLx MySQL 行，由 SeaORM 原生驱动代码进行列序、索引与类型解码；仅用 ProxyExecResult 构造原始计数/ID 回执，不用 BTreeMap 代理行猜测模型类型。默认 DML 适配仍关闭 SeaORM SQLx 驱动；仅启用本特性时打开原生行转换，MySQL 核心仍无 SeaORM 依赖。直接复用已有 SQLx 0.8.6 及维护补丁，未新增依赖版本、安装工具或创建另一条业务连接。

权威现场：[20261128_173042_eb40812f](20261128_173042_eb40812f/result.json)。最终 catalog SHA-256：`37c72132ee4b33449c7a4f22c2bea90cd80d674aae7cea5eee83aea5cdfd16fa`，1402 个哈希成员。Rust 1.99 / 1.85、真实 MySQL、未修改原 Java TC/原 Undo parser；真实 Entity 的插入并返回模型、模型更新、模型删除各做提交/补偿，共十二笔实际全局事务。插入生成主键 101，返回精确四位 DECIMAL、大整数/中文 JSON 和文本；同事务模型查询看到自己的插入/更新/删除，独立外部连接在准备前仍只看到种子。独立验收完整业务字段、原登记/锁/报告/回调、原子 Undo/SHA/历史见证及终态，原 Java parser 重读十二份 Undo。

每个流另实际验证：反向列标签 z/a 的索引 0/1 与原列序保持；DATE/TIME(6)/DATETIME(6)/UTC TIMESTAMP(6) 原生读取；外部提交 v=11 后本事务仍读到原 RR 值 10；九项语法/控制/分页拒绝；选中超 1MiB 字段拒绝而只选主键可读；第 1025 行拒绝并 poison；真实 MDL 阻塞中取消原查询，原事务 poison/关闭，新作用域可继续读取。

双版本全工作区/all-features 各 **850 passed / 0 failed / 418 ignored**，AT/MySQL/SeaORM all-target/all-feature 严格 Clippy 通过，忽略项不计通过。实际原生测试另经 --ignored --exact 执行；编译失败测试在完整回归日志中通过。新增读取识别器测试先因 API 缺失编译失败，原日志封存为 `red-missing-select-api.log`。

首轮 [172826](20261128_172826_365b4970/result.json) 已通过读取/快照/结果预算检查，随后夹具用预处理协议执行 LOCK TABLES，被 MySQL 1295 拒绝，不计完成。修订轮仅将夹具锁表/解锁命令改用原始命令协议，再实际通过取消检查和全部事务。原失败记录与自有进程清理完整保留。

```powershell
python -B tools/formal-source-junit/native_seaorm_borrowed.py
python -B tools/formal-source-junit/validate_seaorm_borrowed.py docs/phase-2/evidence/seaorm-borrowed/20261128_173042_eb40812f --full
python -B tools/formal-source-junit/validate_seaorm_borrowed_regression.py docs/phase-2/evidence/seaorm-borrowed/20261128_173042_eb40812f
```

原生运行输入、二进制哈希、原帧/SQL/Undo/Java 重编码、回归命令与原日志、默认/全特性依赖树、审计及生成脚本均封存。新现场须另按封存命令执行回归与汇总后再作完整验收。完整 lock 审计退出 1，保留 time/SQLite 两项告警，无 ignore；正常 all-features 图未激活它们和 rsa。root/关闭 TC 认证的回环现场不作为生产权限或认证证明。

完整 ORM 仍开放：关系/连接、聚合与完整分页、LIKE 等更多表达式、流式读取、嵌套事务、迁移、2.x 和各物理类型/配置的完整认证。生成键批量、完整 SQL/全部写入器、生产门禁、WP07 和 Phase 5–7 不因本增量关闭。

后续 [原生单表统计/分页](../seaorm-pagination/README.md)已补齐 COUNT(*)、一层受控统计派生表与只读 LIKE；双版本 3006 行分页、分页流和十二笔实体原 TC 事务通过，全工作区各 853 passed / 419 ignored。上述初始读取范围中的这部分缺口已有证据，通用聚合/关联、更多表达式及完整生态继续开放。
