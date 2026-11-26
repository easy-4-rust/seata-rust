# WP05 原主键锁键与事务锁集合

[最终执行](run_20261116_011215_dc9906b8/result.json)落实[锁键契约](../../global-lock-key-contract.md)的格式与有界集合部分；[机器状态](../../wp05-global-lock-keys.json)、[只读终核](readonly-review-20261116.json)。没有关闭原WP05/06或全部后续计划。

7984冻结输入/9141成员，catalog SHA256=`3975d1900d85da9c00cef46f5b190516953cf69de230b96d5ad27f4ae45cac07`。双Rust1.99/1.85各新执行325工作区测试（316既有+9新增），96测试产物、12实际RM通用恢复子进程；8普通实现削弱与1个真实数据库逆序PK削弱均编译后命中目标断言。178份既有控制/数据源生产文件及7056个依赖输入保持原字节，185registry版本/checksum不变，150实际编译包、6871源码；DB仅增加既有serde_json版本的dev edge。

|本轮新执行边界|实际结果|
|---|---|
|原方法格式/次序|1029项原Java/Rust MATCH，含原四组测试值探针；原JUnit没有执行|
|Java HashMap次序|容量、碰撞树、根移前、扩容拆分/退链、混合hash；1003个原生成整数次序案例纳入普通回归|
|Rust1.99现场|[实际运行](../mysql-lock-key/run_20261116_011746_443b5310/result.json)，2配置×9实际准备日志锁键MATCH|
|Rust1.85现场|[实际运行](../mysql-lock-key/run_20261116_011821_cad0c2ce/result.json)，2配置×9实际准备日志锁键MATCH|
|原有Undo对照|每版同一现场重新比较4条实际prepared codec日志的原Java类型/值/容器；40文档双向及再次40MATCH另行重跑|
|隔离负控|8普通目标；现场故意改为SQL索引序，真实逆序PK断言返回2_1而非1_2，只有1个声明源文件被改动|

3个自有MySQL容器均清理，D盘保留数据目录。其余38个数据库场景每版只继承前次字节不变的DB执行路径，不称本轮重跑39。MySQL8.4.13/lower_case_table_names2及测试环回非TLS范围不扩大为生产认证/TLS证明。

## 实现与关键区别

`BaseTransactionalExecutor`保留原表名、镜像行序、Java字段字符串形式；`ConnectionContext`选DELETE前镜像/其他后镜像、完整语句键去重、稳定连接，失败不改变集合/计数/配置。两对象仍PARTIAL，不含原完整执行器、保存点、XID绑定或分支状态。

原`TableMeta#getPrimaryKeyOnlyName`实际把PRIMARY索引列插入默认Java HashMap再遍历；它与真实SQL主键索引序可能不同。目标RED曾把(b,a)索引编码为2_1，而原方法返回1_2。新增有界固定JDK17次序复现，包含碰撞树与resize；数据库SQL/Row主键索引序未改。真实16列碰撞主键表也在现场对照中通过，不能把这一Rust扩展算作完整Java HashMap迁移。

整数不经float，DECIMAL保留原BigDecimal.toString的scale/科学计数法，二进制为原Seata ArrayUtils有符号byte列表。JDBC profile显式选择，默认signed tinyint(1)仅物理0/1可无损变Boolean；2须numeric或拒绝。实际TINYINT主键、DECIMAL主键、二进制主键、逆序复合主键与碰撞主键均被读取和比较。

单键/累计输出1MiB、128日志，写入String前检查并计表前缀/分隔符/二进制展开；巨大人工元数据只用于预算测试，不声称其DDL受MySQL支持。一个事务锁集合固定一个JDBC profile；混用配置的实际RED已修复，空日志factory也固定配置，失败不绑定未成功配置。Debug/错误不输出主键值。

原Java新编译BaseTransactionalExecutor、ConnectionContext、ArrayUtils三份固定源；旧18源/32class构建继续绑定，完整fallback/helper字节封存。额外oracle运行时明确固定config-core、Typesafe config1.2.1、json-common-core及netty-common4.2.12，全部file加载来源受核对，不声称重建完整原Maven环境。早期缺JsonUtil/Netty、测试域/scale错误和测试局部变量遮蔽都按setup诊断保留，不算生产行为RED。

## 开放门槛与下一步

格式MATCH不证明TC锁覆盖。原TC空白PK、分号提前结束和二进制列表逗号拆分已有[专门核查入口](../../tc-lock-coverage-investigation.md)；隔离原方法录制后端观察是未完整资格化的诊断，没有真实TC网络请求/真实锁存储证明。外部TC风险未接受，不能改原键字节或自动接受限制来关闭门禁。

原189文件/1135方法声明仍需完整具体案例展开/迁移，四个来源值probe不变成原JUnit完成。Source assets见[原生成库存](../../lock-key-fixture-ledger.json)，原字节保护独立于Java测试资产完整迁移。BranchRegister、同连接Undo原子写入、提交未知持久围栏/恢复、公开COMMIT与所有后续计划继续OPEN；旧38数据库场景与旧组件控制各保留其历史范围，RM通用恢复不证明SQL恢复。

复现见[工具说明](../../../../tools/formal-lock-key/README.md)，所有开发/运行输出只在D/E盘；无分支切换、提交/推送或发布。
