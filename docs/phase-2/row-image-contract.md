# WP05 精确行镜像数据模型契约

延续[MySQL基础契约](mysql-foundation-contract.md)及原WP05/06。本增量先实现Java Field/Row/TableRecords的有界、不可变数据模型；数据库行读取、before/after与业务执行随后接入。三对象均为部分迁移；Java可变Bean、任意JDBC对象及原序列化格式不自动成为Rust兼容承诺。

- Field保留原列元数据、主键属性与精确值。FieldValue是Java Object值域的Rust扩展，区分NULL、i64、u64、UTF-8文本、任意二进制及十进制；不经过f64/JSON number。NULL与空文本/空BLOB不同；值必须符合列类型、可空、最大octet长度和DECIMAL精度/scale。
- DecimalValue采用规范普通十进制字符串，不接受指数、加号、空白、前导零或负零；保留末尾小数零与scale，最多65位有效定点容量、scale最多30。列绑定时要求scale恰好匹配并满足整数位容量，DECIMAL(p,p)的整数0不占有效整数位。没有Java BigDecimal/序列化互通声明。
- 每个字段payload最多1MiB，Row必须包含完整物理列序、全部字段元数据与同一原表名；不接受任意投影、缺列、重复列、错序或相似但不相同的schema。primary_keys按真实索引序返回，non_primary_keys按物理序返回；不把Rust字节排序冒充MySQL文本排序规则。
- TableRecords固定一次元数据；空记录有效。增加行前验证schema、行数与累计payload字节，失败保持原记录不变。上限1024行/8MiB payload，可显式下调至正数，不能上调；整数payload按8字节、NULL按0、其余按实际UTF-8/二进制/十进制文本字节计。列名/元数据/容器固定开销另由128列、64字节标识符和行数上限约束，不声称payload预算等于总堆内存。
- Debug和错误不包含字段值；所有构造与累积入口执行预算，禁止先克隆超长输入再检查。本地值容器接受调用方已拥有的输入，不证明SQLx在接收网络包前已限长；真实数据库读取还必须先长度/行数预检且全程持锁。
- 本模型不自行生成TC锁键、不按Rust相等性判定数据库collation唯一性、不暴露业务提交。实际SQL排序、锁键转义、原SQL参数、镜像完整性、Undo格式/校验/原子性及未知提交仍保持原验收要求。

当前ColumnType表示五类值域，尚未保留TINYINT/INT等整数位宽、CHAR/VARCHAR字符数限制或DECIMAL的UNSIGNED属性。Field成功仅证明上述内存值域、NULL、octet和十进制维度不变量，不是完整MySQL列合法性或可安全回写证明；SQLx解码/补偿前须补足这些物理维度并实际验证。

验证：整数极值、65位DECIMAL/30小数位、UTF-8字节、NULL/空值、类型与完整schema拒绝、独立PK序、累计预算原子失败、Debug脱敏；Rust1.99/1.85、受影响回归及有目标断言的隔离削弱。没有真实MySQL行镜像或Java具体案例差分的完成声明。
