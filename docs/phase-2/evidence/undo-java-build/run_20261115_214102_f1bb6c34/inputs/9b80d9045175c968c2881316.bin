# WP05 原Java Undo格式与类型身份

延续[WP05实施契约](wp05-mysql-at-plan.md)及[业务/镜像契约](mysql-execution-images-contract.md)。原Java固定cba73e180fd8896a99df6415b859d275122feaa7；当前有界执行和内存SQLUndoLog已验证，不表示可持久化或可提交。本增量先建立原格式对照，再接入编解码及同事务Undo写入，原十四项及WP06门槛保持。

## 原格式对照

- 使用原BranchUndoLog/SQLUndoLog/Field/Row/TableRecords及JacksonUndoLogParser源码，保留原包名、类型标签、集合封装、SQLType和JDBC整数类型码。固定实际依赖、编译输入和加载产物，不以旧target目录的存在推断其来源。
- Jackson版本先按固定源码依赖声明2.18.3建立明确对照配置；早期server/lib的2.21.2探查仅为未认证样本。驱动分别记录原依赖5.1.42与原image配置8.0.27，不能拿已有8.0.28缓存静默代替。若声明配置在当前数据库不适用，保留失败原因，不伪造成功或悄悄换驱动。
- 在自有MySQL8.4.13随机库里，用原MysqlTableMetaCache读取元数据及原TableRecords.buildRecords取行，再由原parser编码/解码。原JDBC类型及运行时值类别均须记录：有符号/无符号整数、精确DECIMAL、字符/二进制长短类型、NULL、空值及复合PK。数据库、驱动和连接属性共同限定对照范围。
- 原空内容`{}`及常规分支/SQL日志各自保留原语义。Java对象默认可变及宽松反序列化不能成为Rust接受任意类名的理由；Rust只处理已声明格式中的类型标签，拒绝不支持/歧义/越界内容。

## Rust映射与预算

- 保留真实JDBC列类型身份，不能仅凭五种FieldValue类别区分CHAR/VARCHAR/LONGVARCHAR、BINARY/VARBINARY/LONGVARBINARY或整数包装类。旧ColumnMeta构造路径保持兼容；缺少认证身份时不得凭猜测编码Java日志。
- 整数与DECIMAL不得经过f64。保留DECIMAL scale、完整u64、任意二进制字节和NULL/空值；原JSON字段顺序与集合顺序分别核实，元数据transient行为按原parser保留。确定性编码、版本上下文、校验/完整消费及资源预算继续原验收要求。
- 输入/输出及嵌套、日志/行/列、单字段和总payload分别受控；超限不发布部分结果。类型/格式错误不输出原SQL或业务值。未知字段、重复字段、类型标签、数值边界及尾随内容必须有明确测试。
- 先以原Java生成资产验证Rust解码，再让原Java解码Rust编码并比较真实字段/类型/值，不能只做Rust roundtrip。源案例与资产逐项记录，未覆盖项继续MISSING/PARTIAL，不以新增少量场景声明全部Java案例完成。

## 接入顺序与开放边界

编解码及原上下文通过后，在同连接/同本地事务写入原undo_log唯一身份记录；在原BranchRegister、持久围栏及提交未知恢复完成前，保持没有公开COMMIT。普通独占锁继续原计划，共享锁按用户要求暂缓，外部TC风险未接受。生产TLS、权限配置和其他数据库/驱动仍需各自验收。
