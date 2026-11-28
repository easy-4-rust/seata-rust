# WP05 原TC排他锁覆盖核查契约

延续[核查步骤](tc-lock-coverage-investigation.md)、[锁键契约](global-lock-key-contract.md)及原WP05/06。固定Java pin、原server JAR、驱动和Rust生产源不改变。共享锁继续暂缓，外部TC风险没有被接受。

1. 来源方法层：真实编译原AbstractLockManager并实际调用collectRowLocks/acquireLock，以录制后端核对返回值、收集到的行和实际委托次数。完整源、fallback、classpath、class加载来源及运行前后字节须封存。它只能证明该方法边界，不能证明网络登记或实际锁存储。
2. 网络层：在自有原Java TC实例使用正式Rust TM/RM和真实原请求身份；普通相同行键冲突作正控制，然后测试空/空白、多行含空值、分号截断、原二进制列表和不同二进制值的冲突语义。用独立帧观察者绑定XID/BranchId、每次原请求、结果与回调，不能只读取客户端PASS。
3. 同一案例结束原全局事务、核对锁释放并清理自有实例。每个事务Begin、每个登记申请及全局结束最多一次，不用重发补造成功；丢失响应保持未知，Finished不视为历史提交成功。资源夹具只记录二阶段调用，不声称SQL/Undo效果。
4. 只把已观察事实列为observed，不把预期的缺陷现象通过写成safety gate通过。字段格式MATCH、来源方法调用、网络登记、实际锁存储和业务原子性分别列证据范围。完整源JUnit仍未迁移。
5. 与实际MySQL主键/生成器结果连接后，才对提交流程的可支持身份、候选修复或后续协议作具体决策；不静默转义/hash原值，不扩大用户已接受的外部限制。原Undo持久化、登记/围栏、公开COMMIT与全部后续目标继续。

## 正式 AT 登记前的行键语法校验

实际准备路径使用原编码生成每个业务行的锁键，并在拼接行分隔符前检查该行可由固定 Java TC 完整解析。单行必须是一个非空白的原 token，不能含逗号或分号。Java 空白以固定 JDK 的 `Character.isWhitespace(char)` 为准。多字节二进制列表因原编码带逗号而拒绝，单字节列表保留原编码。复合键仍按原顺序拼接，再检查完整行 token。

任何一个业务行不能完整传输时，原准备作用域整体失败并回滚，本次分支注册不发送，既有全局事务仍由调用方结束。通过校验的原锁键字节保持一致，原格式生成器继续用于原 Java 差分。该校验证明一行能进入原 TC 的解析结果。后台锁键长度、资源别名、排序规则、复合键身份碰撞以及完整排他覆盖仍按原计划验收。

## 复合文本主键登记校验

原格式使用下划线连接主键分量。主键包含两个及以上文本分量时，只要原值也包含下划线，Rust 正式登记准备便拒绝整个本地事务。该规则按主键类型和原编码判断，可能覆盖列宽约束下只有一种切分的键。单个文本主键、只有一个文本分量的混合主键以及值不含下划线的复合文本主键继续按原字节编码。追加登记上下文同样检查每条 Undo。原格式生成器保留 Java 字节，供诊断和差分使用。

拒绝发生在发送 BranchRegister、登记恢复状态和写 Undo 前。历史现场仍记录原 Java TC 对两组不同复合主键的额外锁冲突。物理资源别名、存储排序规则、完整排他覆盖和公开业务 COMMIT 仍按原验收追踪。 [登记校验验证](wp05-tc-lock-composite-registration.json)

## 正式 RM 与原 TC 的排他锁现场验证

Rust 1.99 和 1.85 各使用正式 AT 准备路径、固定原 Java TC 和 MySQL 8.4.13 DB 锁存储完成一次联试。每次独立观察到普通整数主键对应的锁行，并用原登记响应绑定 XID、BranchID、资源 ID 和行键。原业务事务本地回滚后，另一事务对同一行的锁查询返回 false，实际分支登记收到 `LockKeyConflict`。第二次独立查询仍只见所有者的原锁行。两个原全局事务各回滚一次，所有者回调完成后，全局事务、分支、锁及业务表的目标行数均为零。

实际 MySQL 写入空串和含分号的键各一次，均在正式登记前失败。原始帧仅有上述普通键的两次登记。现场运行使用按仓库源码重建的隔离认证夹具，原 server JAR 及 Connector/J 仍按固定哈希核对。完整排他锁覆盖、公开业务 COMMIT 和共享锁继续按原验收范围跟踪。 [现场验证记录](wp05-tc-lock-native-registration.json)

## 原 TC 的 DB 锁字段上限和排序规则

严格模式下，原 TC 对 `pk` 的 36 个四字节 Unicode 字符、`table_name` 的 32 字符和 `row_key` 的 128 字符都成功登记。分别加一字符时，原响应以异常码 18 拒绝并指明对应超长列。每个案例单独触及一个字段上限。独立 SQL 快照核对成功锁的原值，并确认超长登记没有增加锁行。这里的长度是该 MySQL 列的字符计数。

业务表的 `utf8mb4_0900_as_cs` 允许同一事务写入 `Cafe` 和 `café`。原 TC 锁表采用 `utf8mb4_0900_ai_ci`，第一个值持锁时，第二个值的锁查询返回 false，随后登记被明确拒绝。两个 Rust 工具链各完成四次成功登记、四次明确拒绝和八次原全局回滚。最后锁表清空，每个成功分支各有一条 Undo 围栏与一条终态恢复记录，业务表无残留。早期一次运行另捕获了一次原 TC 重复回调，保留原始帧及其独立重验记录。

这些结果覆盖固定原 TC 的 MySQL 8.4.13 DB 存储配置。其他 SQL 模式、排序规则、物理资源别名及复合键身份碰撞仍按 WP05 验收。 [字段边界与排序规则验证记录](wp05-tc-lock-storage-boundaries.json)

## 原版 MySQL 锁表长度校验

以原 Java TC 的 `script/server/db/mysql.sql` 为准。连接这个 MySQL DB 锁存储的 Rust AT 管理器可通过 `MySqlOptions::with_java_tc_mysql_lock_limits()` 启用预检。默认保留其他存储后端和定制表的既有行为。校验按存储列的 utf8mb4 字符数计算 `resource_id` 256、`table_name` 32、`pk` 36，以及 `resource_id^^^table_name^^^pk` 128。逐行检查多行和多语句锁集合，补充平面字符按一个字符计算。

超限在发送 BranchRegister 前返回本地预算错误并回滚，已通过的原锁键字节保持一致。此配置依赖现场采用原版表定义，部署方仍须核对实际 TC 后端和表结构。业务与 TC 排序规则差异、其他客户端与完整排他锁验收仍待覆盖，公开一阶段 COMMIT 保持禁用。 [长度验证](wp05-tc-lock-db-length-guard.json)

## 业务文本主键的排序规则校验

原版 Java TC 采用 MySQL DB 存储且 `lock_table.row_key` 为 `utf8mb4_0900_ai_ci` 时，构建正式准备管理器所用的 `MySqlOptions` 可启用 `with_java_tc_mysql_lock_collation()`。这是管理器选定的目标存储配置，默认关闭。需要在所有使用该保障的管理器上明确启用，部署方须独立核对 TC 锁表真实排序规则。业务主键或 TC 锁表排序规则发生迁移时，先停写并排空旧 XID。事务在同一连接持有业务表元数据锁，准备时核对每个涉及的文本主键列。排序规则取自当前物理列的已解析类型，表名拼写遵循数据库实际的大小写规则。空表及删除后的表仍能读取列类型。校验只在有 Undo 的登记准备中运行。

实际 MySQL 8.4.13 中，`utf8mb4_general_ci` 的主键列把 `key` 和 `key ` 判定相同，而原 TC 的 NO PAD 锁表区分二者。原固定 TC 现场观察到两个全局事务同时拥有两条锁。启用后两次不匹配请求均在登记前失败，匹配排序规则的重音变体仍由 TC 拒绝冲突。准备阶段会保留原键，不做空格裁剪、折叠或转码。现场用例回滚原业务事务，后续删除由夹具模拟已释放本地行锁的状态。 [验证记录](wp05-tc-lock-collation-guard.json) [MySQL 排序规则说明](https://dev.mysql.com/doc/refman/8.4/en/charset-unicode-sets.html)

任意排序规则的兼容性、跨客户端一致性及完整排他锁和公开业务 COMMIT 仍待验收。

## 可选的物理 schema 资源绑定

在业务库应用 `005_seata_rs_at_resource_binding.sql` 后，由部署账号在 `slot=1` 写入该 schema 唯一获准的原 JDBC 资源 ID，使用二进制绑定参数。运行账号只读该表，业务 SQL 中此表名保留给管理端。MySQL 8.0.22 起，`FOR SHARE` 可只授予 SELECT。连接池使用自有会话，禁止部署端注入同名临时表。所有负责该 schema 的 Rust AT 写入器启用 `MySqlOptions::require_at_resource_binding()`；事务或准备管理器任一端启用即可要求校验。默认保留旧配置行为。 [MySQL 锁定读说明](https://dev.mysql.com/doc/refman/8.0/en/innodb-locking-reads.html)

有 Undo 的准备在同一业务连接按字节比较资源 ID，并持有 InnoDB 的共享行锁与元数据锁直到该本地事务结束。缺表、缺行、ID 不符或无法确认物理表时，在登记前失败并回滚。映射要在停写并排空旧登记和回调后调整。生产连接池与回调必须稳定指向同一物理库，不能用这个表判断 DNS、代理路由、主从数据库或复制的绑定行具有同一事务身份。

物理引擎还须从原事务 MDL 下的当前建表定义复核，不能只依赖先前读取其他业务表时形成的字典快照。迟到改为 MyISAM 或替换为视图时，在发送分支登记前拒绝并关闭原作用域。校验复用原业务连接与期限，不增加辅助连接，不扩大为完整绑定表列/约束规范认证。

部署方须在所有写入器之间统一原 JDBC ID，并排空旧 XID。无法发送相同资源 ID 的客户端不能加入此锁域。通用 RPC 客户端不执行数据库校验。该扩展只覆盖启用了校验的 Rust 准备，完整 TC 安全门槛继续待验收，公开业务 COMMIT 保持禁用。 [绑定验证](wp05-tc-lock-resource-binding.json)

对固定原版 Java TC MySQL DB 锁存储，可用 `MySqlOptions::with_java_tc_mysql_db_lock_profile()` 一次启用资源绑定、原版锁表字段长度和文本主键排序规则三项既有登记前校验。它不自动创建 005 绑定表、探测 TC 后端或修改 TLS；启用前仍须核对 TC 实际表结构/排序规则、同一物理主库及各写入器的原 JDBC ID。单项配置继续可用。该组合降低配置遗漏，不能据此判定外部客户端或原 TC 完整排他锁安全。

组合配置与固定原 Java TC 的选定身份场景已经现场执行。历史 `utf8mb4_0900_as_cs` 复合键夹具被组合排序规则保护正确拒绝；组合模式专用夹具改为与原 TC 锁表一致的 `utf8mb4_0900_ai_ci` 后，两 Rust 工具链各执行六组资源别名/复合键案例，原报文、独立锁表及终态均通过。各有三次登记（两成功、一冲突）、两次锁查询、六次原全局回滚和三次登记前拒绝；原 TC 回调重试次数不同，均绑定原身份且无最终锁残留。此结果只证明固定场景，不能推广至任意外部写入器或关闭公开业务 COMMIT 门槛。[组合联试记录](wp05-java-tc-mysql-db-profile.json)

## 跨连接池准备时保留已启用的锁校验

当业务事务和准备管理器使用相同原 JDBC 资源 ID 的不同连接池时，登记前逐项执行任一池已启用的资源绑定、锁表长度和文本主键排序规则校验。排序规则及绑定继续在持有业务表元数据锁的原事务上核对。验收须独立配置两个池，证明超长键或不兼容排序规则导致本地拒绝，原 Java TC 未收到这些键的 BranchRegister，合法登记仍能完成并释放锁。

仅在业务事务数据源启用时，修复前不兼容排序规则的键获得原 TC 成功登记回执，37 字符锁键收到原 TC 的 `FailedStore`。两次失败运行保留原始帧和客户端断言。 [排序规则失败记录](evidence/tc-lock-collation-guard/seata_original_guard_ruuzpc7i/client-stdout.log) · [超长键失败记录](evidence/tc-lock-db-length-guard/seata_original_guard_b9qhhtwj/client-stdout.log)

| Rust | 仅事务数据源启用排序规则 | 仅准备管理器启用排序规则 | 仅事务数据源启用长度 | 仅准备管理器启用长度 |
|---|---|---|---|---|
| 1.99 | [结果](evidence/tc-lock-collation-guard/seata_original_guard_hik0gh4t/result.json) | [结果](evidence/tc-lock-collation-guard/seata_original_guard_knmwz77_/result.json) | [结果](evidence/tc-lock-db-length-guard/seata_original_guard_nkz1mbl2/result.json) | [结果](evidence/tc-lock-db-length-guard/seata_original_guard_d5rk_5_9/result.json) |
| 1.85 | [结果](evidence/tc-lock-collation-guard/seata_original_guard_b1wl5gpu/result.json) | [结果](evidence/tc-lock-collation-guard/seata_original_guard_p6fxjwae/result.json) | [结果](evidence/tc-lock-db-length-guard/seata_original_guard_7ijs6i9c/result.json) | [结果](evidence/tc-lock-db-length-guard/seata_original_guard_404214ql/result.json) |

每组排序规则运行拒绝两个 PAD SPACE 锁键，保留兼容排序规则上的正常登记和原 TC 冲突响应。每组长度运行拒绝三个超限键，恰达边界的键仍可登记。固定 Java TC 的原始帧和独立 MySQL 查询确认超限或不兼容键没有申请锁，合法分支结束后业务和锁计数归零。

组合配置的命令行入口同时启用物理别名场景。双工具链各复跑六个场景，均完成三次原登记、三次登记前拒绝、六次全局回滚，最终释放所有锁。 [Rust 1.99](evidence/java-tc-mysql-db-profile/seata_original_guard_125aj_lx/result.json) · [Rust 1.85](evidence/java-tc-mysql-db-profile/seata_original_guard__sughfbu/result.json)

## 同一 JDBC 资源的物理实例一致性

连接池从首个 MySQL 会话读取 `@@server_uuid` 和 `DATABASE()`。实际使用该池的业务和恢复连接必须落到同一 MySQL 实例与同一 schema。携带 Undo 的准备在发出 BranchRegister 前比较事务来源与准备管理器的实例身份。005 的 JDBC 绑定行仍用于跨写入器约束资源名。连接池实例身份只在本池生命周期内有效，重建或故障切换须遵循停写和旧 XID 排空流程。现场须由两个不同 UUID、同名 schema 及相同绑定内容的真实实例证明前置拒绝。

两个隔离的 MySQL 8.4.13 实例通过同一个回环 JDBC 地址接入，使用相同的 schema 与 005 行。修复前，切换后端后原池获得了指向另一实例的连接。将另一实例上的业务事务交给原实例的管理器时，受控 TCP 对端实际收到一次 BranchRegister，原实例新增一条恢复行。修复后，原池拒绝切换后的连接，路由恢复时原池可继续开始事务。跨池准备明确在本地拒绝，原包对端观察到零次 BranchRegister，业务、Undo、恢复行均为零。 [修复前](evidence/tc-lock-physical-identity/run-1.99.0-94f2f2d9140e/physical-identity.json) · [Rust 1.99](evidence/tc-lock-physical-identity/run-1.99.0-d839dcbcd257/physical-identity.json) · [Rust 1.85](evidence/tc-lock-physical-identity/run-1.85.0-12a8ef8b7f63/physical-identity.json)

该现场覆盖新物理连接的路由变化。代理在一条已建立的 MySQL 会话内更换后端、复制出的相同 UUID 和新进程的跨实例恢复仍受部署约束。

固定原 Java TC 的完整组合锁配置在双工具链上各复跑六个场景，Rust 1.99 的实验性一阶段业务 COMMIT 和原 TC 报告联试也通过。 [组合配置 1.99](evidence/java-tc-mysql-db-profile/seata_original_guard_u3vx3_ip/result.json) · [组合配置 1.85](evidence/java-tc-mysql-db-profile/seata_original_guard_e8n1a3c4/result.json) · [原 TC 提交联试](evidence/java-tc-changed-commit-report/run-commit-1.99-6abc02bd/verified.json)

## 原 Java AT 持锁时 Rust 竞争同一行

两个客户端连接同一固定 Java TC 的 MySQL DB 锁存储实例，通过同一 JDBC 资源更新 `accounts` 的整数主键 1。原 Java `DataSourceProxy` 将余额从 1000 更新为 900，实际提交本地事务、Normal Undo 和 TC 分支登记，然后把全局事务留在进行中。正式 Rust TM/RM 启用组合锁配置，查询同一锁得到 false，实际更新同一行时，准备入口向 TC 登记原锁键并收到 `LockKeyConflict`。独立 MySQL 快照核对原锁仍属于 Java，余额仍为 900，且原 Undo 正文逐字节保留。隔离夹具关闭 TC 注册认证以接入未经替换的 Java 客户端。

Java 全局提交后，独立 SQL 确认锁和 Undo 已清理。Rust 的下一原 XID 重新更新该行并取得锁，随后本地回滚并由原 TC 发起全局回滚回调。最终余额保持 900，只保留 Rust 分支的 `GlobalFinished`，全局、分支和锁表都为空。观察器保留 Java V2 和 Rust V1 真实报文并按原 XID、BranchId、资源和完整锁键交叉核对。该场景覆盖 Java 持锁、Rust 竞争的整数主键路径。

Rust 1.99 和 1.85 均通过现场核对。 [Rust 1.99](evidence/tc-lock-java-rust/seata_original_guard_js6zw76a/result.json) · [Rust 1.85](evidence/tc-lock-java-rust/seata_original_guard_r8gwcaw6/result.json)

## Rust AT 持锁时原 Java 竞争同一行

同一固定 Java TC 与 MySQL DB 锁存储下，Rust 通过实验性一阶段提交将 `accounts:1` 的余额从 1000 改为 990，提交 Normal Undo 和 004 见证并成功报告 `PhaseOneDone`。在 Rust 全局事务结束前，原 Java 客户端开启显式 JDBC 事务，对同一行更新后在原连接读到待提交余额 890。夹具将 Java RM 的重试次数设为零，原 TC 返回 `LockKeyConflict`。Java 的原 `ConnectionProxy.commit` 回滚该 JDBC 事务，同一连接随后读到余额 990。前后的独立 MySQL 快照核对 Rust 的锁、余额和 Undo 字节完全相同。

Rust 全局提交清理分支锁及 Undo 后，新的 Java XID 对同一资源与锁键完成本地和全局提交。最终余额为 890，TC 会话、分支、锁及 Undo 全部清空。该场景沿用隔离夹具的注册认证配置，覆盖整数主键的反向竞争。其余主键类型、外部写入器与公开业务 COMMIT 继续按 WP05 验收。

Rust 1.99 和 1.85 均通过现场核对。共核对三个不同 XID、一次 Rust 成功登记、一次 Java 明确冲突及释放锁后的一次 Java 成功登记。Rust `PhaseOneDone`、全局提交与分支提交回包均通过原始报文关联核验。 [Rust 1.99](evidence/tc-lock-rust-java/seata_original_guard_1x5vn3al/result.json) · [Rust 1.85](evidence/tc-lock-rust-java/seata_original_guard_q5jt5i6j/result.json) · [原 Java 持锁场景回归](evidence/tc-lock-java-rust/seata_original_guard_ni_kg_i1/result.json)

## 混合类型复合主键的双向竞争

原 Java 与 Rust 复用双向持锁场景，使用 `PRIMARY KEY(id,segment)`。独立 MySQL 查询核对物理索引顺序、`segment` 的 `utf8mb4_0900_ai_ci` 排序规则，以及 Java 的 `NORTH` 和 Rust 的 `north` 都命中 `(9007199254740993,'north')`。原 Java V2 与 Rust V1 都登记 `accounts:north_9007199254740993`。锁键沿用 Java 的主键遍历顺序，两个分量保留原字符与精确整数。

Java 持锁时 Rust 登记收到冲突并保留 Java 的更新。Rust 持锁时 Java 提交收到冲突，并在原连接上看到本地余额恢复。两种场景都在释放后用新 XID 成功登记。同行号不同区段与同区段相邻行号的两条记录，在各次独立快照中保持各自的余额。该验证覆盖所测 BIGINT 与同排序规则 VARCHAR 组合，其余主键形状及生产提交条件仍按原 WP05 验收。

| Rust | Java 持锁 | Rust 持锁 |
|---|---|---|
| 1.99 | [验证](evidence/tc-lock-java-rust/seata_original_guard_lm7gci0b/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_zuq13bpk/result.json) |
| 1.85 | [验证](evidence/tc-lock-java-rust/seata_original_guard_97jpruk0/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_2k1d8ri7/result.json) |

## DECIMAL 主键的双向竞争

固定 Java TC 和 MySQL DB 锁存储上，两端用实际 JDBC 和 SQLx 事务竞争两类 DECIMAL 主键。独立 MySQL 查询核对主键精度、刻度和不同精确十进制字面量命中的物理值。Java 使用较短的等值谓词，Rust 使用完整刻度。两端的原 BranchRegister 字节分别核对为以下锁键。

| 物理主键 | 两端登记的锁键 |
|---|---|
| `DECIMAL(38,8)` 的 `9007199254740993.12500000` | `accounts:9007199254740993.12500000` |
| `DECIMAL(30,20)` 的 `0.00000010000000000000` | `accounts:1.0000000000000E-7` |

两种数据各含比目标值小和大一个最小刻度单位的邻近行。Java 或 Rust 持锁时，另一端都对同一原锁键收到锁冲突。释放后，新 XID 成功登记。独立 SQL 快照确认邻近行保持各自的原值，Java 的被拒绝提交在原 JDBC 连接上恢复了持久余额。这个场景沿用隔离夹具的注册认证及 Java 零锁重试配置。其他键类型及生产提交条件继续按 WP05 验收。

| Rust | 大数 Java 持锁 | 大数 Rust 持锁 | 小数 Java 持锁 | 小数 Rust 持锁 |
|---|---|---|---|---|
| 1.99 | [验证](evidence/tc-lock-java-rust/seata_original_guard_mswyonfj/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_52xd_rqg/result.json) | [验证](evidence/tc-lock-java-rust/seata_original_guard_iualr1sv/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_7048zpo6/result.json) |
| 1.85 | [验证](evidence/tc-lock-java-rust/seata_original_guard_x963txfl/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_d6wvxm7i/result.json) | [验证](evidence/tc-lock-java-rust/seata_original_guard_kf269myo/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_8a7rf2eq/result.json) |

## VARBINARY 主键的排他锁边界

同一个 `VARBINARY(2)` 主键列含 `FF`、`FE` 和 `00FF` 三个不同原值。原 Java 用十六进制字面量更新 `FF`，Rust 绑定原二进制字节。两端登记同一个有符号字节列表 `accounts:[-1]`，独立 MySQL 查询确认 TC 锁表只有一个完整键。原 TC 在两个持锁方向拒绝竞争方，释放后新的全局事务可完成登记。两个邻近键的余额均保留。

Rust 持有 `FF` 的分支锁期间，另一个本地事务实际更新 `00FF`，并因原列表 `[0, -1]` 含逗号而在准备阶段被拒绝。原始 RM 报文只有先前的单字节登记与一次成功报告。独立 SQL 确认双字节行仍是原值，唯一 Normal Undo 和锁归属于单字节分支。历史原 TC 观察另记录了把 `[0, -1]` 当作两个锁片段的行为。本次夹具覆盖可完整登记的单字节值，以及多字节值在正式 Rust 准备路径上的拒绝。 [原 TC 拆分证据](evidence/tc-lock-live/README.md)

| Rust | 原 Java 持锁 | Rust 持锁及双字节拒绝 |
|---|---|---|
| 1.99 | [验证](evidence/tc-lock-java-rust/seata_original_guard_kwpsk1wm/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_03uj6qep/result.json) |
| 1.85 | [验证](evidence/tc-lock-java-rust/seata_original_guard_kg6fuwmz/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_g0l19iid/result.json) |

## TINYINT(1) 的 JDBC 配置与锁键

固定 Java TC 和实际 MySQL 的同一业务表使用有符号 `TINYINT(1)` 主键，独立 SQL 核对物理值 0、1 和 2。Boolean 配置下，原 Java Connector/J 的 `tinyInt1isBit=true` 与 Rust 默认配置都把物理值 1 登记为 `accounts:true`。数字配置下，`tinyInt1isBit=false` 与 Rust 的显式数字配置都把物理值 2 登记为 `accounts:2`。两种配置各自执行 Java 持锁和 Rust 持锁的双向竞争，释放后新 XID 成功登记。独立快照确认其他物理行值保持一致。

Boolean 配置的 Rust 持锁用例另对物理值 2 执行实际更新。Rust 准备阶段识别到这种 Boolean 转换会丢失信息，拒绝登记并回滚该次更新。原 RM 帧只有物理值 1 的有效登记，独立查询确认物理值 2 保留原余额。

| Rust | Boolean，Java 持锁 | Boolean，Rust 持锁 | 数字，Java 持锁 | 数字，Rust 持锁 |
|---|---|---|---|---|
| 1.99 | [验证](evidence/tc-lock-java-rust/seata_original_guard_95kdv35_/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_xlu8icab/result.json) | [验证](evidence/tc-lock-java-rust/seata_original_guard_57n2b0ip/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_xg_da8od/result.json) |
| 1.85 | [验证](evidence/tc-lock-java-rust/seata_original_guard_v8zya4xu/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_bfhjhpf0/result.json) | [验证](evidence/tc-lock-java-rust/seata_original_guard_dv8prtwg/result.json) | [验证](evidence/tc-lock-rust-java/seata_original_guard_tpj9tlqh/result.json) |

混用对照对物理值 1 使用同一原 JDBC 资源。Java 保持 Boolean 映射并持有 `accounts:true`，Rust 数字映射实际查询 `accounts:1`。原 TC 返回可锁定并接受 Rust 分支，独立 SQL 同时看到 `true` 和 `1` 两条锁记录。Rust 本地写入随后回滚，只保留 Java 已提交的业务更新。该对照复现排他缺口，生产写入器的准入仍须验证所有客户端的 TINYINT(1) 映射一致。

[混用对照 Rust 1.99](evidence/tc-lock-java-rust/seata_original_guard_97n8vu0y/result.json) · [混用对照 Rust 1.85](evidence/tc-lock-java-rust/seata_original_guard_ak_k4bll/result.json)

### Rust 写入器的 TINYINT(1) 映射绑定

在业务库应用可选迁移 `011_seata_rs_at_tinyint_profile_binding.sql`，为原 005 资源绑定行增加 `tinyint1_is_bit`。部署方在停止写入并排空旧 XID 后设置 `slot=1` 的映射。布尔配置填写 `1`，数字配置填写 `0`，旧行迁移后为 `NULL`，等待显式配置。

使用 `MySqlOptions::require_at_tinyint_profile_binding()` 的 Rust 写入器在携带 Undo 的分支申请前，把准备管理器的 JDBC 配置和持锁绑定行比较。业务事务数据源和准备管理器任一端启用即生效。缺少迁移、映射为 `NULL` 或不匹配时，准备失败并回滚本地工作。匹配时，绑定行的共享锁保留至本地事务结束。005 资源标识约束可独立使用。

原 Java 持有 Boolean 锁期间，Rust 数字映射的本地更新在登记前被拒绝。Java 全局提交、旧 XID 排空后，绑定值设为 `NULL`，新的 Rust 事务仍在登记前拒绝。明确设置数字映射后，新 XID 成功登记。此时独立连接尝试改写映射，MySQL 返回 1205。随后 Rust 回滚并释放锁，原始 RM 报文只出现一次成功登记，持久日志只有一次 Pending 和一次 Completed。修复前，绑定行已配置为 Boolean，但旧代码仍取得数字分支的成功回执。 [修复前记录](evidence/tc-lock-java-rust/seata_original_guard_hmgvuap6/client-stdout.log)

| Rust | 仅业务事务数据源启用 | 仅准备管理器启用 |
|---|---|---|
| 1.99 | [验证](evidence/tc-lock-java-rust/seata_original_guard_uniqbljk/result.json) | [验证](evidence/tc-lock-java-rust/seata_original_guard_v12b3mlc/result.json) |
| 1.85 | [验证](evidence/tc-lock-java-rust/seata_original_guard_93ntpb_7/result.json) | [验证](evidence/tc-lock-java-rust/seata_original_guard_v8oiqre3/result.json) |

这一校验适用于启用该选项的 Rust 写入器。原 Java 客户端的 Connector/J 映射需要在部署时独立核对。异构写入器准入及公开业务 COMMIT 的验收继续开放。

## Java 写入器事务准入

固定原 Java AT 客户端可在显式本地事务开始后、第一条业务 SQL 前，对将要写入的原 `ConnectionProxy` 调用兼容层。校验通过后继续在该连接上写入和提交。校验读取当前物理 MySQL server UUID 和 schema、原 Seata JDBC 资源 ID、实际 Connector/J 的 `tinyInt1isBit`，并在当前事务对 005/011 绑定行执行 `FOR SHARE`。预配的物理身份及绑定由部署账号提供。缺失或不匹配即拒绝，调用方回滚原事务。运行账号只需读取绑定表，部署账号管理绑定。变更前停写并排空 XID。

该入口针对接入此校验的固定原 Java 版本和 Connector/J。部署方仍须落实写入权限隔离、稳定路由以及全部写入器的同一锁域配置。公开业务 COMMIT 和完整 WP05 门槛继续开放。

失败对照中，原 Java 数字映射未执行此校验。独立 SQL 观察到 `accounts:2` 原锁、已提交余额变更及 Normal Undo。启用后，数字映射、指向同一物理 schema 的 JDBC 别名和错误预配 UUID 均在业务 SQL 前被拒绝。独立 SQL 的原余额、Undo 和锁表均无变化，原始 Java 帧中没有这些失败写入器发出的 GlobalBegin 或 BranchRegister。

匹配配置通过校验。另一 MySQL 会话在原 Java 本地事务仍持有绑定行共享锁时修改映射，得到 1205。Java 随后在原连接执行并提交业务 SQL，使用 `accounts:true` 登记。Rust 启用同一 011 绑定校验，竞争同一行时收到原 TC 明确锁冲突。Java 全局提交后，新的 Rust XID 成功登记并回滚，保留原防悬挂记录。两个工具链均核对原 Java/Rust 线缆报文和独立 SQL。现场测试使用自有 MySQL 容器的夹具账号。权限隔离仍需以真实部署角色和其他写入器验证。

[未接入校验的失败对照](evidence/java-tinyint-writer-admission/seata_original_guard_phi1e9gy/wrong-mapping/database.tsv) · [Rust 1.99](evidence/java-tinyint-writer-admission/seata_original_guard_w48akhph/result.json) · [Rust 1.85](evidence/java-tinyint-writer-admission/seata_original_guard_6v9qs3nr/result.json) · [Java 接入说明](../../compatibility/seata-java/writer-admission/README.md)

### 分离写入账号的现场验证

使用不同账号执行 Java 本地业务提交、Rust 锁竞争和 Rust 回滚，另设仅查询业务表的账号。两个写入账号都只有 005/011 表的 SELECT。MySQL 实际拒绝两端改写映射、ALTER 绑定表和创建同名临时表，也拒绝只读账号直接更新业务余额。绑定最终仍为预配的原资源 ID 和 Boolean 映射。

Rust 只在实际业务表、Undo 和分支恢复表拥有直接表级 `TRIGGER`。本次仍使用全局 `PROCESS`。服务端日志确认受限 Java 账号执行 1 次业务更新、3 次绑定读取，受限 Rust 账号执行 2 次业务更新、2 次绑定读取。Java 持锁时，Rust 竞争得到原 TC 锁冲突。Java 结束后 Rust 成功登记并回滚。原始报文和独立 SQL 核对了原锁及 Undo 终态。此结果覆盖当前 UPDATE 夹具和这些账号授权。其他写入器、生产元数据通道及公开业务 COMMIT 继续验收。

[Rust 1.99](evidence/at-writer-permissions/seata_original_guard_o6kdqvwf/result.json) · [Rust 1.85](evidence/at-writer-permissions/seata_original_guard_ns61ckbt/result.json) · [表级元数据验收](mysql-foundation-contract.md#表级触发器可见性)

分离账号的反向竞争也已执行。Rust 用受限账号提交真实业务并持有 `accounts:true`。Java 在同一绑定下先收到原锁冲突，后在 Rust 释放锁后成功登记。独立 SQL 绑定原 006/007 记录、004 见证和二阶段提交终态。[Rust 1.99](evidence/at-writer-permissions/seata_original_guard_yps9wiz6/result.json) · [Rust 1.85](evidence/at-writer-permissions/seata_original_guard_8sv4j8zf/result.json)

## 跨行跨表登记的冲突原子性

同一物理 MySQL 库中，原 Java AT 先提交 `accounts:1` 的本地更新及 Normal Undo，并保留全局锁。Rust 在一个本地事务中执行两条实际 UPDATE，写入 `accounts` 的两行以及 `reserves` 的一行，向原 TC 提交一条 `accounts:0,1;reserves:1` 登记。该申请包含三个物理行键，其中 `accounts:1` 已被 Java 持有。原 TC 返回明确锁冲突。Rust 发出全局回滚以前，独立 SQL 确认失败方的 XID 仍为 Begin，分支数为零，锁表只保留 Java 原锁，未提交的 Rust 业务和 Undo 均不可见。

Java 仍持锁时，新的 Rust XID 对 `accounts:0;reserves:1` 查询得到可锁定，实际登记成功。其全局回滚释放两条锁。Java 随后全局提交并释放原锁，第三个 Rust XID 再提交完整锁集合并获得三个物理锁。验收核对三个 Rust 原登记请求、各自回包、实际锁表和成功分支对应的回滚回调。最终余额仅保留 Java 的业务变化，Rust 两个成功登记的分支各保留一条防悬挂记录。该场景沿用隔离夹具的注册认证配置。

[Rust 1.99](evidence/tc-lock-bundle/seata_original_guard_zffgzgiz/result.json) · [Rust 1.85](evidence/tc-lock-bundle/seata_original_guard_5pgx98xg/result.json) · [单行场景回归](evidence/tc-lock-java-rust/seata_original_guard__peagk6i/result.json)

## 重叠锁批次的并发登记

两次真实的 MySQL UPDATE 分别生成 `accounts:1,3` 和 `accounts:2,3`。两个本地事务随后回滚，释放业务行锁。两个不同 XID 同时向固定原 Java TC 发送 BranchRegister。测试夹具只在自有数据库的 `lock_table` 安装触发器。独有键插入后标记工作线程，公共键 `3` 写入前等待同一个命名锁。独立连接在 `performance_schema` 看到两个不同的 TC 数据库线程分别完成独有键插入并等待公共键。此时锁表中没有已提交行。

释放同步点后，原 TC 恰好接受一组完整锁，另一组收到 `LockKeyConflict`。失败 XID 仍为 Begin 时，独立 SQL 只看到获胜分支及其两条锁，原全局锁查询确认失败方的独有键可锁定。两条登记请求均先于第一个登记响应。随后回滚两个 XID，原 TC 只向获胜分支发送回滚回调。最终 TC 会话、分支和锁全部清空，三条余额保持原值，只有获胜分支留下一条 `GlobalFinished` 防悬挂记录。

这组证据覆盖原 Java TC 并发批量登记时的锁原子性。外部写入器准入和公开业务 COMMIT 仍待验收。

[Rust 1.99](evidence/tc-lock-bundle-race/seata_original_guard_ihgu4i5x/result.json) · [Rust 1.85](evidence/tc-lock-bundle-race/seata_original_guard_64biflqd/result.json) · [原跨表场景回归](evidence/tc-lock-bundle/seata_original_guard_prqv4yvx/result.json)

## 原 TC 的物理资源别名与复合键碰撞

同一 MySQL 容器将 3306 映射到两个回环端口。两个正式 Rust 数据源读取到相同的 MySQL server UUID、schema 和预置业务行，经各自连接对同一行执行实际 UPDATE。在第一条原全局锁仍持有时，同一资源 ID 的对照事务查询返回 false 且登记冲突。使用另一个 JDBC 资源 ID 查询返回 true，登记成功，独立 SQL 在同一时点读到两条资源 ID 不同的锁行。夹具中的本地业务写入均回滚。

固定复合主键表可在一个本地工作事务中容纳 `('a_b','c')`、`('a','b_c')` 和 `('a','b_d')`。正式准备路径把前两组编码为相同的 `identity_pair:a_b_c`，后者查询返回 false，原 TC 明确拒绝登记。第三组生成 `identity_pair:a_b_d` 并成功登记。两套工具链各核对六次登记、四次锁查询、六次全局回滚及四个分支的独立终态和 Undo 围栏，保留了 TC 重试回调的原始帧。

这两项分别留下物理排他锁缺口和额外锁冲突。WP05 完整锁安全门槛继续开放，兼容键格式及资源 ID 的处置继续沿用原验收约束。 [身份边界记录](wp05-tc-lock-identity-boundaries.json)

## 完整 JDBC 配置的可选严格准入

TINYINT 布尔位相同不足以代表 UTC/精确 DATE/TIME 表示相同。新增[完整配置绑定契约](mysql-jdbc-profile-admission-contract.md)，以可选 021 列和 `require_at_jdbc_profile_binding()` 核对完整码，事务池或管理器任一端启用均执行。原默认、资源绑定和 TINYINT 入口不变；旧全局分支排空后才允许维护方变更绑定。原 TC 八笔事务和双 Rust 真实配置/共享锁矩阵已验证这一守卫；完整写入器协作、权限、排他覆盖与生产门禁继续按 WP05 推进。
