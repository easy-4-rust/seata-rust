# MySQL 数据源与镜像基础契约（WP05实施中）

沿用[WP05实施契约](wp05-mysql-at-plan.md)，不改十四项验收与WP06。当前SQL识别已验证；本增量落实真实数据库连接、本地事务内元数据与镜像读取。业务写入、Undo与提交恢复仍需后续实际证明，不能以元数据成功关闭原子性Gate。

## 实际边界

- 既有镜像ID=`sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902`，inspect显示MySQL8.4.13系列；现场记录`SELECT VERSION()`后才能确认运行版本。端口仅127.0.0.1，自有随机库/容器，D盘独立数据目录。
- SQLx固定候选0.8.6，关闭默认features，启用MySQL、Tokio、rustls-ring/webpki；锁文件与Rust1.85实际通过后才能称可用。密码/SQL参数不写入Debug或公开错误，SQLx语句日志关闭。TLS默认校验身份；仅明确选择的loopback测试选项可关闭TLS，生产TLS/认证验收仍未完成。
- 新增`seata_datasource`存放ColumnMeta/TableMeta及后续Row/Field/TableRecords等公共对象；`seata_db_mysql`负责具体连接与元数据。按Java对象分文件，当前仅为相关Java类的部分实现，不宣称完整迁移。
- 所有连接预算有限，设置等待/语句/锁超时，UTF8MB4、严格模式、NO_BACKSLASH_ESCAPES与REPEATABLE READ会话。设置后实际读取校验，不能仅相信配置已提交。外部共享数据库不参与测试或迁移。

## 元数据与事务

数据源允许显式提供 1..65536 字节的 PEM CA 列表，并始终保持 VerifyIdentity。配置入口检查容量并解析 PEM，要求至少一个证书块；DER 证书结构、信任链、有效期和服务器主机身份由固定 SQLx/rustls 在连接握手时验证。所配置 CA 补充固定驱动内置的 WebPKI 信任根。自定义 CA 与关闭回环测试 TLS 的选项双向互斥，配置 Debug 不展示证书正文。连接池和当前约束辅助会话复用同一原连接配置，保持相同的 TLS 校验与原操作期限。

客户端身份由成对的 PEM 证书链和未加密 PEM 私钥配置，两者分别限制为 1..65536 字节。配置阶段要求至少一个证书块及可识别的 PKCS#1、PKCS#8 或 SEC1 私钥块，DER 结构、证书用途和密钥匹配关系在握手时校验。该身份不放宽服务端 VerifyIdentity，与关闭回环测试 TLS 的选项双向互斥；公开 Debug 与错误不展示证书或私钥正文。身份通过原连接配置传递给连接池和当前约束辅助会话。本接口不承诺驱动内部密钥副本的零化，也不提供加密私钥密码入口。

固定 MySQL 8.4.13 的选定 REQUIRE X509 账号已完成真实认证验证。缺客户端证书、其他 CA 客户端、证书与私钥不匹配以及错误密码均被拒绝；正确身份完成池、辅助会话、元数据及业务回滚，并独立确认原值。原 MySQL 客户端使用 VERIFY_IDENTITY 认证同一账号并记录 TLS 1.3，不提供客户端身份时返回 1045。Rust 1.99、1.85 各通过 635 项工作区测试、两项现场 TLS 测试及严格 Clippy，移除身份设置的有效削弱控制命中目标断言。此记录只覆盖当前 RSA/PEM 和直接账号配置，生命周期、轮换/吊销、其他算法及生产最小权限继续验收。 [客户端认证验证](wp05-mysql-mtls.json)

选定 RSA 配置的有效期及在线轮换已补四项现场验证。真实 X509 账号拒绝过期和未生效客户端证书。MySQL 对失效服务端证书重载返回 3888，并保留旧合法 TLS 上下文；SQLx/rustls 直接拒绝失效服务端证书的路径由连接真实 MySQL 的双 TLS 前端验证，前后合法基线及削弱控制确认该边界。真实 MySQL 的同 CA 换叶证书可供新池和辅助连接使用，换 CA 后旧信任拒绝新握手并污染原事务，回滚恢复原值；同一双 CA 配置在两端均可建立新元数据会话。既有会话不因换证追溯失效，客户端信任更新通过新配置和数据源执行。本轮生产 API、既有 TLS 测试及锁文件摘要与前一验收一致。Rust 1.99、1.85 各通过四线程调度下的四项现场测试、635 项工作区测试及严格 Clippy。 [生命周期与轮换验证](wp05-mysql-tls-lifecycle.json)

固定 MySQL 8.4.13 的选定服务端 TLS 配置已完成现场验证。正确 CA 与匹配 IP SAN 可以执行元数据和可回滚业务，缺 CA、错误 CA、错误主机名及明文连接均被拒绝。独立别名连接与 server UUID 核对排除 DNS/IPv6 失败造成的假阳性，独立 MySQL 客户端记录强制加密及 TLS 1.3。Rust 1.99、1.85 各通过 634 项工作区测试、现场 TLS 用例和严格 Clippy，两项有效削弱控制命中目标断言。其他证书链、生命周期、客户端证书和生产认证配置继续按原计划验收。 [TLS CA 验证](wp05-mysql-tls-ca.json)

固定原 Java TC 联试进一步验证了 MySQL 强制 TLS 下的事务准入。Rust 1.99 和 1.85 均在无关 CA 握手失败后保留原 XID 的 Begin 状态，当时业务、Undo、分支、锁及恢复记录均为空。正确 CA 放行后，同一 XID 完成原分支的登记、成功报告和提交回调。独立 SQL 核实了 Java TC 的 TLS 会话以及提交后的持久状态。 [原 Java TC 联试](evidence/java-tc-changed-commit-report/README.md#mysql-tls-信任失败与原事务提交)

客户端每次connect（含初始化/profile）、begin（含借用/BEGIN）、完整table_meta链和rollback使用单一绝对期限，默认3秒、可配置1ns..60秒；不能用服务端语句超时替代，也不能逐SQL重置总预算。SQLx0.8.6的PoolConnection在进入拥有式Transaction::begin之前标记close_on_drop；当前基础阶段结束事务后丢弃该物理连接，避免取消后的协议状态被重用。其关闭路径自身有固定5秒上限；公开pool close最多等待3秒，超时仍返回未排空错误。该保守关闭策略尚非吞吐优化完成。

固定原 Java TC 已覆盖默认构建中无变化 COMMIT 的限时退出。两 Rust 工具链均观察到原会话从存活到关闭，SQLx 在原服务器提交后发出清理命令。同一个容量为 1 的业务池随后在另一物理会话执行实际 UPDATE 并确认回滚，独立 SQL 核对原业务状态。[连接池回收联试](evidence/java-tc-changed-commit-report/README.md#无变化-commit-超时后的连接池回收)。

固定原 Java TC 的有变化 COMMIT 截留场景也在两套 Rust 工具链验证了连接池回收。原业务连接结束后，同一个容量为 2 的池在两条新物理会话上同时持有租约，两个探测事务都确认回滚，随后原分支按持久见证恢复并完成提交。[现场证据](evidence/java-tc-changed-commit-report/README.md#有变化-commit-超时后的连接池回收)。

table_meta在首次await前暂时将事务所有权移入其Future：取消会直接丢弃事务/连接，保留外层poison；正常错误仍将事务归还poison作用域供显式rollback，超时丢弃事务而不声称远端已完成回滚。部分撤权（SHOW GRANTS中的REVOKE）当前明确未支持，不能被全局授权覆盖。

1. 原表名先经ASCII/长度/引号规则验证；标识符按规则加反引号，值全部绑定。拒绝跨schema和`undo_log`作为普通业务表。
2. BEGIN后先通过`SELECT 1 FROM <table> LIMIT 0 FOR UPDATE`取得元数据锁，再在同连接查询information_schema。元数据锁持有到本地事务结束，真实DDL阻塞测试必须证明该边界。
3. 必须是InnoDB BASE TABLE，有1..128列和完整非空主键。保留表/列原大小写、物理列顺序、PK索引顺序、类型/unsigned/nullability/precision/scale/最大长度。列名按MySQL列名规则去重；PK顺序不能用物理列顺序猜测。
4. 当前拒绝视图、无PK、非事务表、触发器、入向/出向外键、生成/不可见/自动更新列、自增列及未知类型。支持整型精确有符号/无符号、文本、二进制、DECIMAL的元数据；时间/浮点等未认证类型明确拒绝，原完整能力目标保留。
5. information_schema可能受权限过滤，不能将不可见触发器或跨库入向FK解释为不存在。每次元数据读取须先证明当前用户对当前表具备直接 TRIGGER 或 ALL PRIVILEGES 可见性，可通过当前表、其 schema 或全局直接授权；直接读取路径另需直接全局PROCESS（或ALL），通过INNODB_FOREIGN读取未按应用表权限过滤的入向/出向关系。仅由角色继承的授权、其他表授权和缺少全局可见性的配置明确拒绝，不自动提权或更改应用账户。PROCESS要求是当前基础实现的权限限制，尚不是生产最小权限方案；生产接入前仍须完善受信元数据通道和相应验收。现场测试包含权限不足拒绝。
6. 元数据快照不提供跨事务永久有效的缓存。业务执行必须在持有相同本地事务/元数据锁时使用；失败后不能继续使用部分结果。事务取消/Drop依赖SQLx回滚机制，但仍须真实验证锁最终释放。

## 镜像与后续提交

镜像要按完整PK排序、在本地事务内锁定行，并使用WHERE参数在原SET参数后的正确切片。大字段与过多行必须在读取完整值前作长度/行数预检；同事务锁持有下再读取完整数据，防止客户端先分配超限BLOB。数据库返回精确i64/u64/文本/二进制/DECIMAL，禁止经f64中转。元数据通过不等于这些镜像要求已实现。

后续业务与Undo始终在同一本地事务；仅明确COMMIT成功后报告PhaseOne_Done。COMMIT未知不得重放SQL；缺Undo行不能单独证明旧提交不可能迟到。Java唯一键/GlobalFinished防悬挂必须结合真实并发与故障验证。当前基础读取阶段不暴露绕过Undo的业务提交捷径。

参考固定版本[SQLx连接](https://docs.rs/sqlx/0.8.6/sqlx/mysql/struct.MySqlConnectOptions.html)、[MySQL8.4元数据锁](https://dev.mysql.com/doc/refman/8.4/en/metadata-locking.html)、[锁定读](https://dev.mysql.com/doc/refman/8.4/en/innodb-locking-reads.html)。完整V1、生产认证、WP00-R、原Go默认与全部后续Phase未完成；外部TC风险未接受，共享锁暂缓。

## 跨表 DDL 与主键快照

REPEATABLE READ 事务先访问表 A 后，其他连接可以在它尚未访问表 B 时修改 B。MySQL 8.4.13 现场确认，后续的 information_schema、SHOW COLUMNS 和 SHOW INDEX 可能继续呈现旧主键。读取器在锁定 B 的 MDL 后，使用当前 SHOW CREATE TABLE 的主键定义核对字典顺序。主键发生变化时拒绝并污染原事务，整体回滚后新事务可读取新定义并正常执行。

此核对针对固定 MySQL 8 建表输出及当前支持的普通主键。额外索引选项或未知输出会保守拒绝。列类型、字符集、引擎、触发器及外键独立变化仍待继续验收。 [现场验证](wp05-metadata-cross-table-ddl.json)
类型独立变化的现场回归已补齐。主键不变而普通列由 INT 改为 BIGINT UNSIGNED 时，读取器通过当前建表定义逐列核对原 COLUMN_TYPE，拒绝旧字典类型并污染原事务。新事务可正常读取并准备 u64::MAX 的镜像，整体回滚恢复原值。字符集、可空性、列数量和顺序、引擎及触发器的独立变化仍待覆盖。 [类型变化验证](wp05-metadata-type-ddl.json)

存储引擎独立变化已补现场回归。即使旧字典仍报告 InnoDB，读取器也从当前建表定义核对实际引擎，MyISAM 在业务 DML 前被拒绝；原事务保持污染并回滚，恢复 InnoDB 后新事务可执行并回滚。包含换行及伪造引擎文本的列注释也已纳入双版本现场验证。 [引擎变化验证](wp05-metadata-engine-ddl.json)

字符集独立变化已补现场回归。读取器在持有原业务表 MDL 的连接上，用空标量子查询的 CHARSET 核对物理字符集与快照。UTF-8 改为 ASCII 后拒绝旧事务，新事务读取 ASCII 域并可执行和回滚。每个字符列增加一次受现有操作期限约束的查询，同字符集内排序规则独立变化仍待完整验收。 [字符集变化验证](wp05-metadata-charset-ddl.json)

列数量、列顺序和可空性的独立变化已补现场回归。原连接通过 SQLx describe 重新准备 SELECT * LIMIT 0，比较当前结果列与旧字典元数据；数量、顺序或可空性失配时拒绝并污染原事务，整体回滚后新事务正常读取。新增不可见列不出现在 SELECT * 中，Extra/生成列相关变化仍待验证。 [列结构验证](wp05-metadata-shape-ddl.json)

不可见列、整数生成列、带独立字符集声明的文本生成列和自增主键的独立变化已补现场回归。当前建表定义的完整列数量用于补足 SELECT * 不包含不可见列的缺口；生成属性在字符集声明之后也会检查；新增 AUTO_INCREMENT 在物理列属性阶段被拒绝。校验基于固定 MySQL 8 的标准建表输出，不识别的结构继续拒绝。 [额外属性验证](wp05-metadata-invisible-ddl.json)

新增触发器及入向/出向外键的跨表 DDL 已补现场回归。原事务先取得目标表 MDL，再使用同认证配置的新会话读取当前约束，并核对双方 server UUID 与 schema。业务执行、前镜像及公开元数据读取三条入口受原绝对期限覆盖，失败或取消保持事务污染并关闭辅助连接。每次检查临时增加一个池外连接，最多与活动业务事务数相同；部署仍要求稳定单主数据库路由。当前证明覆盖检查前已提交的约束变化，检查后外键新增的锁时序及独立 Undo 补偿读取路径继续待验证。 [约束变化验证](wp05-metadata-constraint-ddl.json)

检查后的约束 DDL 锁时序已补固定 MySQL 8.4.13 现场验证。原事务完成公开元数据检查后，另一会话分别执行新增触发器、出向外键与带 DELETE CASCADE 的入向外键，三者都返回服务端锁等待超时 1205。原事务随后仍可执行 UPDATE 和 DELETE 并整体回滚，独立查询确认原值、子表行和副作用表。原事务释放后，同一 DDL 成功，新事务按当前约束拒绝并保持污染。该证据覆盖本次标准 DDL 与回滚释放时序，独立 Undo 补偿入口及其他数据库版本仍待验证。 [检查后 DDL 验证](wp05-metadata-postcheck-ddl.json)

## 表级触发器可见性

Rust 以实际目标表匹配 MySQL `SHOW GRANTS` 的直接授权。业务表、Normal Undo 和分支恢复表各自核对当前对象。读取当前触发器和外键的辅助连接也先验证权限。直接全局 `PROCESS` 继续用于跨库入向外键，部分撤销继续拒绝。

失败对照中，仅持有目标表 `TRIGGER` 的账号在实际 MySQL 上被旧实现拒绝。修复后，目标表可正常读取，带触发器的表和缺少对应授权的表分别被拒绝。Rust 1.99 和 1.85 各执行 18 项真实 MySQL 元数据回归。[失败对照](evidence/mysql-metadata/run_20261126_014241_0bb03758/live-tests.log) · [Rust 1.99](evidence/mysql-metadata/run_20261126_014945_f933275d/result.json) · [Rust 1.85](evidence/mysql-metadata/run_20261126_015022_12259608/result.json)

## 受信数据库约束通道

固定 MySQL DB 配置可部署 012 的定义者权限存储过程 `seata_rs_at_constraint_flags_v1`。部署方在目标 schema 安装它，并向应用账号直接授予该过程的 EXECUTE。过程定义者使用锁定登录的专用身份，持有目标 schema 的直接 TRIGGER、全局 REFERENCES 和本过程的 EXECUTE。Rust 核对定义者授权，拒绝部分撤销或视图不完整的结果。运行账号通过原连接读取当前表是否存在触发器、入向或出向外键。直接具备当前表触发器及全局外键可见权限时，沿用原直接读取路径。

过程限定当前 schema 内的单个表，返回定义者授权和两个约束标志。业务及恢复作用域仍持有原 MDL，辅助连接继续验证原物理服务器和 schema。过程的代码、定义者、授权及替换由部署账号管理，运行账号不得维护该过程。收紧或切换写入权限前按原流程停写并排空 XID。

MySQL 8.4.13 的外键字典按引用方表执行访问过滤，因此过程需要覆盖其他 schema 的引用方。过程使用定义者身份读取字典，Rust 核对该身份返回的直接授权。[固定 MySQL 字典视图](https://raw.githubusercontent.com/mysql/mysql-server/mysql-8.4.13/sql/dd/impl/system_views/referential_constraints.cc) · [MySQL 存储对象授权说明](https://dev.mysql.com/doc/refman/8.4/en/stored-objects-security.html)

旧实现真实拒绝缺少表级 TRIGGER 的运行账号。改用受信过程后，两个 Rust 版本均找到运行账号不可见的触发器和跨 schema 入向外键。两种权限削弱均明确失败，分别是定义者的外部 schema REFERENCES 被部分撤销和本 schema TRIGGER 被移除。Rust 1.99 和 1.85 各通过 20 项真实 MySQL 元数据回归。[旧实现对照](evidence/mysql-metadata/run_20261126_020106_991a0cd8/live-tests.log) · [Rust 1.99](evidence/mysql-metadata/run_20261126_021043_35fb543c/result.json) · [Rust 1.85](evidence/mysql-metadata/run_20261126_021122_67654330/result.json)

固定原 Java TC 和自有 MySQL 的联试使用直接过程授权。Rust 写入账号无全局 PROCESS 及任何表级 TRIGGER，直接访问 InnoDB 外键字典和修改存储过程均被 MySQL 拒绝。原 006 在缺少过程授权时仍拒绝读入，通过授权后，原 Rust 分支完成实际业务提交、成功报告和二阶段回调。Java 竞争收到原锁冲突，释放后用新 XID 成功登记。[Rust 1.99](evidence/at-definer-metadata/seata_original_guard_iexffs5v/result.json) · [Rust 1.85](evidence/at-definer-metadata/seata_original_guard_66ndc1v0/result.json)

Rust 1.99 的直接授权提交路径另行通过回归。[原直接授权回归](evidence/at-writer-permissions/seata_original_guard_5f5igpzi/result.json)


## 原生 JSON 元数据（2026-11-28）

在已有类型之外新增独立 JSON 物理域与原生身份，无字符维度，禁止主键。原 JDBC 8.0.27 现场为 -1 / String；仅对实际 JSON 原生行读取 UTF-8 字节，保留其他类型检查。见 [JSON 契约与证据](mysql-json-contract.md)。前文有日期的类型清单保留其历史范围。
