# 单行自增主键与原回执镜像

对应 [正式契约](../../mysql-auto-increment-contract.md)、WP05 和 SeaORM 生成键前置要求。正式元数据保留自增属性，旧八参数构造默认 false；当前物理定义与 RR 字典快照必须一致。现有 JDBC 类型配置和预算内的单列整数自增主键进入正式执行路径，复合或非主键自增仍拒绝；没有修改 Java Undo 字节结构。

INSERT 缺省自增键、NULL 或默认模式的零，由同一原业务语句的非零 OK ID 定位完整后镜像。显式键沿原服务器类型赋值路径，显式完整键的批量插入继续支持；需要生成多个键的语句在业务 SQL 前拒绝。未查询 MAX 或会话 LAST_INSERT_ID 来替代原回执，也不推算自增序列。新增 `MySqlOptions::with_no_auto_value_on_zero()` 保留显式零；DELETE 补偿恢复零键时，仅在原拥有式补偿租约上保留 NO_AUTO_VALUE_ON_ZERO，租约按原策略关闭。默认严格会话和提交授权不变。

权威现场：[20261128_165513_52181b49](20261128_165513_52181b49/result.json)。最终 catalog SHA-256：`08004ff0b3506feeae2a13aeb496ed74fb15e67d59e7c0b0d78d8dbd01ece79c`，2186 个哈希成员。未修改原 Java TC / 原 Jackson Undo parser、真实 MySQL 8.4.13、Rust 1.99 与 1.85；每版十三类 DML 各执行提交/补偿，共 **52 笔实际全局事务**，全部原 Java parser 读取并重编码一致。独立验收核对原资源与完整锁键、原登记/报告/回调、全业务行、原子 Undo/SHA/历史见证、恢复终态及合法围栏。

实际服务器使用步长 7 / 偏移 3。覆盖缺省/NULL/零/表达式零/显式零保留/显式键/显式批量、有符号自增、无符号键 `9223372036854775810`、另一个连接先分配 101 后本分支取得 108、UPDATE/DELETE 及零键 DELETE 的补偿。两版补偿均将零键精确恢复为零。显式键单行原 OK ID 为 200，显式批量为 300；回执不应被强行改为零或统一解释为批量首键。

每个流另验证三类拒绝：缺省键批量、NULL/显式键混合批量、自增复合主键。独立 Performance Schema 插入计数均为 1→1，保持种子行，拒绝发生在业务 INSERT 前，不能用事后回滚掩盖实际执行。两版还实际运行既有物理 DDL 测试，新增/移除 AUTO_INCREMENT 的双向旧字典快照均拒绝、保持 poison、回滚原事务；原不可见/生成列拒绝继续成立。

双版本全 workspace/all-features 各 **845 passed / 0 failed / 417 ignored**；datasource、MySQL、SeaORM 的 all-target/all-feature 严格 Clippy 通过。新增四个组件测试覆盖旧构造兼容、元数据主键约束、NULL/零模式分类、批量拒绝、原始 u64 无损及计数/零 ID/整数溢出拒绝。忽略项未计为通过，原生用例另经 --ignored --exact 实际执行。

初始反例 [164804](20261128_164804_40e06130/result.json) 在真实服务器上因旧物理自增拒绝而失败，证明原行为缺失；[165231](20261128_165231_68ef6f47/result.json) 前五类通过，随后测试错误地预期显式自增插入的 OK ID 为 0，被真实 200 回执拒绝。两份 incomplete_evidence、原帧、原日志和拥有式进程清理记录保留，不计入 52 笔最终通过；修订测试预期，没有把实现的原回执改写为预期值。

```powershell
python -B tools/formal-source-junit/native_auto_increment.py
python -B tools/formal-source-junit/validate_auto_increment.py docs/phase-2/evidence/mysql-auto-increment/20261128_165513_52181b49 --full
python -B tools/formal-source-junit/validate_auto_increment_regression.py docs/phase-2/evidence/mysql-auto-increment/20261128_165513_52181b49
```

现场封存回归命令/原始日志及生成脚本；后两个命令核验已封存现场，新现场还须按记录执行对应 workspace/Clippy 并汇总。没有改依赖或安装工具。回环夹具使用 root，TC 测试关闭认证，不能作为生产认证/权限证明。

完整 ORM 仍待原生 ConnectionTrait、查询/模型返回、关联、迁移和 2.x；生成键批量、完整 Java SQL/全部写入器准入、生产门禁、WP07 和 Phase 5–7 继续开放。此处只关闭已列明并实际验证的单行自增主键前置要求，不将 BIGINT 现场泛化为全部物理宽度与 JDBC 配置已认证。
