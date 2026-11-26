# MySQL Undo 执行器：六个原案例

规格事实源：[source-junit-contract.md](../../source-junit-contract.md) 与 WP05。固定 Java Git cba73e180fd8896a99df6415b859d275122feaa7，依赖完整便携基线的可信 manifest 1ac8fcff14d29fd25bf42d61b4db07b421d1a84cc97c87e35fa7f0a237ebefd7。

三类原 MySQLUndoInsert/Update/DeleteExecutorTest 各两个案例，共六例、13 处原断言。原类及 BaseExecutorTest 原字节重新编译、原样执行；另有仅替换断言调用的观察副本，仍委托原 JUnit。12 份原输入快照核对表名、SQL 类别、列名、JDBC 类型、KeyType、PK 列表及前后每行的字段值。

原三个类均使用 SQLType.UPDATE，测试不按执行器名称改写这个输入。JDBC 1 保留为 CHAR，原字符串不转整数。Rust 物理模型需要的 ASCII/CHAR(64) 测试装箱维度在原 mock 中没有提供，仅用于满足已验证物理类型的不变量，不声称是原表定义。镜像选择核验同一对象引用。

正式 SQLx 补偿与原案例使用同一 build_undo_sql/get_undo_rows 方法。六个 Rust 用例各自对应原类文件；参数化 SQL 字符串、绑定顺序、预算与事务控制沿用已有实现。原 Java 测试只断言关键词/列名及镜像相等，本批不宣称 Java SQL 文本逐字节相同、完整 Java 对象 API、JSON/可空输入或全部数据库类型均已迁移。

## 最终证据

- oracle_20261127_031157_0c4a414a：六例原 JUnit 与观察副本均通过；13 处断言及 12 份完整输入快照。记录实际加载来源，最终验证目录另封存原/观察/辅助 class。
- rust_20261127_032738_6c2149fc：Rust 1.99/1.85 各六例、13 处断言逐值对照；各拒绝七项证据削弱。错误镜像、错误 DELETE 动词、错误 INSERT 动词三个生产代码变异均在目标断言失败，恢复原字节后通过。执行的原/变异/恢复测试二进制均按摘要封存。
- live_20261127_031220_2bf25987：固定已安装 MySQL 镜像，两版本各五组真实补偿：INSERT、复合 PK UPDATE/DELETE、混合反序、脏数据整体回滚、已恢复/相同镜像。服务器 Prepare/Execute 记录核对参数绑定顺序与实际值。正式 UPDATE SQL 构建器错误变异由 MySQL 拒绝（SQLSTATE 42000 / 1064），恢复原字节后同一目标通过。结束时原 Undo 与自有物理会话数均为零，自有容器删除。
- regression_1.99_20261127_032823_051321ee 与 regression_1.85_20261127_032823_a667a6ba：全工作区全功能各 723 passed / 318 ignored（含 rustdoc 摘要），MySQL 全目标全功能严格 Clippy 通过。
- validation_20261127_033733_fb69c1ad：只读程序复核上述权威目录、原/观察类来源、原输入、执行二进制、真实 SQL、清理和现行生产代码。真实数据库现场后的单元记录器编码修正仅属于 cfg(test) 单元观察器，不编入集成测试库；排除理由及新旧摘要单列，生产和原案例夹具保持已测字节。

逐案入口：[wp05-mysql-undo-executor-source.json](../../wp05-mysql-undo-executor-source.json)。最终验证 catalog SHA-256：94f2375f3374b3fcf4c23e2fa2d17d1e043b579d7bbacb0ce1c680a3af48394b。

## 重跑

    python -B tools/formal-source-junit/mysql_undo_executor_source.py
    python -B tools/formal-source-junit/mysql_undo_executor_rust.py --oracle <新 oracle 目录>
    python -B tools/formal-source-junit/mysql_undo_executor_live.py
    python -B tools/formal-source-junit/mysql_undo_executor_regression.py --rust 1.99
    python -B tools/formal-source-junit/mysql_undo_executor_regression.py --rust 1.85
    python -B tools/formal-source-junit/validate_mysql_undo_executor.py --inputs tools/formal-source-junit/mysql_undo_executor_qualification_inputs.json

变异运行器在 finally 恢复生产原字节，不与其他 Rust 构建并行运行。只有两个 regression 命令可在无变异时使用独立 target 并行执行。Java 与数据库原 DDL 从已封存便携输入读取，不写原 oracle、不下载或升级工具。

## 诊断与未完成项

早期 oracle_20261127_030555_d5d0fbed 未记录 KeyType，rust_20261127_030739_a6cbfdc1 / rust_20261127_031347_594eb561 属于补强输入或修正单元日志编码前的中间对照。live_20261127_030950_6c6d0db0 的双版本五例通过，目标变异也实际失败，但收集器只检查 stdout、未读取 stderr，未完成恢复验证；保留为诊断。031510 的 1.85 工作区通过而 Clippy 指出测试记录器编码写法，不能算完整严格检查通过。最终权威目录如上，不累加中间运行作为更多原案例。

独立人工/另一 Agent 审查门禁仍开放，程序只读复核不替代它。全局 Rust MATCH 总数未重算，其余原案例、完整 Java 对象值域、WP05 十四项、WP06–07、完整 V1 和后续 Phase 继续。公开有变化业务提交仍按已有实验性门禁管理。
