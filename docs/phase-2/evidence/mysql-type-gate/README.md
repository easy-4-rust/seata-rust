# WP05 原生MySQL类型身份及原Java格式对照

[最终执行](run_20261115_214918_18929a67/result.json)推进[Undo格式契约](../../mysql-undo-format-contract.md)，保留18种原生MySQL类型身份及TINYINT(1)显示宽度；不改变FieldValue或原业务语义。[机器状态](../../wp05-mysql-type-identity.json)。旧构造保持None，设置身份须与物理类别/位宽兼容，后续with_domain会复核已有身份。这里只验证类别一致与真实元数据，不声称手工构造器已验证全部MySQL DDL合法性。

最终7992输入/9275成员，catalog SHA256=`fe2928425b3ec7bd3e725f5b104f948548756f0f6f648869e571103fb13fdc6b`。Rust1.99/1.85各重新执行297普通工作区及38真实MySQL场景，84工作区与10正向现场产物；2普通及2真实数据库削弱均在指定断言失败，现场负控产物独立归档。12既有RM通用文件恢复子进程重新运行，不是SQL/Undo恢复证明。原执行组件11项控制为历史证据，本轮未重跑。

|Rust|场景集|通过|子记录|
|---|---|---:|---|
|1.99|metadata|9|[run_20261115_215035_5e63f6f5](../mysql-metadata/run_20261115_215035_5e63f6f5/result.json)|
|1.99|row_images|10|[run_20261115_215126_9feb1bbe](../mysql-row-images/run_20261115_215126_9feb1bbe/result.json)|
|1.99|image_race|1|[run_20261115_215217_bff82351](../mysql-image-race/run_20261115_215217_bff82351/result.json)|
|1.99|execution|17|[run_20261115_215307_645b01da](../mysql-execution/run_20261115_215307_645b01da/result.json)|
|1.99|mysql_types|1|[run_20261115_215410_d64c0683](../mysql-type-identity/run_20261115_215410_d64c0683/result.json)|
|1.85|metadata|9|[run_20261115_215453_d7a65ff2](../mysql-metadata/run_20261115_215453_d7a65ff2/result.json)|
|1.85|row_images|10|[run_20261115_215535_c0782e67](../mysql-row-images/run_20261115_215535_c0782e67/result.json)|
|1.85|image_race|1|[run_20261115_215625_d0a1796c](../mysql-image-race/run_20261115_215625_d0a1796c/result.json)|
|1.85|execution|17|[run_20261115_215704_20f8bcd0](../mysql-execution/run_20261115_215704_20f8bcd0/result.json)|
|1.85|mysql_types|1|[run_20261115_215747_05786d9e](../mysql-type-identity/run_20261115_215747_05786d9e/result.json)|

38=metadata9+row10+查询间竞争1+执行17+原生身份1，全部是当前源码的新执行。新现场一例逐列核对20列/18种类型，包含有符号/无符号域和独立连接看到的tinyint(1)声明。真实RED曾返回None而非Some(Int)；修复后双版GREEN。去掉native类别校验、取消域更新复核分别命中纯测试；丢掉tiny显示宽度或把TINYTEXT合并成TEXT分别命中真实目标字段断言。正向263共同输入全部与父快照逐SHA绑定，负向仅允许一个声明变更文件，其余输入相同。

185registry版本/checksum保持，148实际cargo-tree编译包与两版compiler-artifact集合一致，6745源码逐crate成员校验；metadata可选图不当实际编译闭包。AT/协议/传输/TM/RM实现未修改，本轮只变更datasource元数据及DB元数据读取，不代表旧Java/Go Gate新执行。

## 原Java格式对照的独立范围

[独立Java审计](java-source-runtime-audit-20261115.json)绑定先前本日完成的build214102/live214333，非父进程重新运行Java。18份选定原源与固定Git提交内容一致，checkout仅行尾差异单独记录，编译实际原字节不改；32class、335实际运行输入及全部file类加载来源逐SHA绑定，生成lambda还绑定已加载owner。完整fallback目录原字节封存，不能称重编译整个原模块。

Jackson2.18.3、Caffeine2.9.3及驱动按原声明固定；其他helper为明确冻结的server/lib配置（如commons-lang3 3.19.0、logback1.5.32），不是完整原POM依赖环境重建。8.0.27两配置各六真实表，调用原MysqlTableMetaCache、TableRecords.buildRecords及JacksonUndoLogParser，完成原encode/decode的类型/值/容器比较；保存16个原生成资产，全部原字节复制到tests/fixtures/undo/java_8_0_27。它们不是189个原测试文件的案例迁移证明。

|原生类别|8.0.27实际JDBC/运行类别|
|---|---|
|整数8/16/24/32位|JDBC -6/5/4；Integer，UINT32为Long|
|BIGINT/UNSIGNED BIGINT|JDBC -5；Long/BigInteger|
|signed TINYINT(1)|默认JDBC -7/Boolean（值2→true）；tinyInt1isBit=false为-6/Integer（保留2）|
|UNSIGNED TINYINT(1)|服务端实际声明tinyint unsigned，两个配置均-6/Integer2|
|CHAR/VARCHAR/TINYTEXT|JDBC 1/12/12，String|
|TEXT/MEDIUMTEXT/LONGTEXT|JDBC -1，String|
|BINARY/VARBINARY/TINYBLOB|JDBC -2/-3/-3，byte[]|
|BLOB/MEDIUMBLOB/LONGBLOB|JDBC -4，byte[]|
|DECIMAL|JDBC3，BigDecimal；scale及65位精度未经float|

原默认两字节{}、TableRecords$EmptyTableRecords具体子类、NULL/空值/任意字节、复合PK和精确极值均被实际比较。5.1.42两个配置都在连接阶段因MySQL8.4移除query_cache_size失败，保留完整失败，不能称该版本已通过。仅本自有容器开启native_password以让旧驱动走到真实失败，不改变应用生产认证。

早期212928仅格式探索且server变量查询有误、来源闭包偏弱；213228因误把初始化临时socket当最终就绪而未执行profile；214115审核器不识别JVM以owner标识lambda，拒绝封存。最后214333改TCP就绪、合法server/plugin查询、全部runtime前后校验及具体原类/默认字节断言，独立复核通过。

本父12个自有Rust数据库容器均清理，Java绑定另一个已清理的自有容器；数据目录留D盘。固定MySQL8.4.13/lower_case_table_names2/loopback非TLS；容器数不是入口初始化mysqld次数。生产TLS与完整权限、Java具体源案例、Rust-Java双向codec、Undo同事务落库/原分支登记/提交未知恢复仍未完成；无公开COMMIT，WP05仍0/14，WP06和所有后续计划继续。外部TC风险未接受，共享锁暂缓。

复现当前原生组件 `python tools/formal-mysql-types/run.py`；Java流程见[工具说明](../../../../tools/formal-undo-format/README.md)。所有开发/运行输出只在D/E盘。


[独立只读终核](readonly-review-20261115.json)通过，未发现新增P1/P2；范围保持原生类型身份与选定原Java格式对照。
