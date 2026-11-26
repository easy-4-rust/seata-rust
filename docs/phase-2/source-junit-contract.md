# WP05：原Java测试执行与逐案例迁移

本文件落实[既有WP05](wp05-mysql-at-plan.md)及原测试迁移底线，不改变已有验收条件。固定Java来源为cba73e180fd8896a99df6415b859d275122feaa7；已有189份Java测试源码、1135个识别到的静态方法声明及7份原始资源只是库存，动态／继承／参数化案例须由真实JUnit引擎展开和执行。

1. 在D盘用cat-file逐blob核对Git对象头与原始内容后导出隔离副本（不依赖Windows archive行尾转换），不修改原oracle、原断言或资产。POM、依赖版本和JDK17参数来自固定源；只读现有C盘Maven缓存，解析产物和下载仅写D盘。
2. 使用原rm-datasource与sqlparser-core模块及其真实Maven reactor依赖，实际编译生产／测试源码；保留命令、类路径、版本、依赖摘要、源与产物绑定。项目依赖下载不等于安装或升级CLI，不更改全局Maven设置。
3. 由原JUnit平台记录发现、禁用、参数化／动态展开及逐案例结果；库存声明数与运行案例数分别报告，失败／未运行不能删除或算通过。原数据源测试需要的真实环境另行由自有依赖提供，未满足时保留失败。
4. 逐案例映射Rust实现和原始资源，保留输入、全部断言、异常、顺序、状态、作用域与清理，再生成可复核MATCH。Java测试运行通过本身不证明Rust迁移通过。
5. 真实JUnit执行、有效反例、双Rust回归及独立只读审查完成后，才发布对应范围证据；未迁移案例与对象继续MISSING／PARTIAL。公开业务COMMIT、TC安全、原全部WP05十四项和完整V1不因本工作自动关闭。

格式化apply属于可修改原源的构建步骤，本基线使用原插件公布的spotless.apply.skip开关保持原字节；不跳过原测试编译或运行、不修改原POM。原Surefire test生命周期与JUnit手动Launcher执行分别记录。

## 当前 RM JDBC 逐案进度

固定版 `JdbcUtilsTest` 的 21 个原始案例均通过。重新编译原始源码与直接运行基线封存的原始 class 各验证了同一组 21 个案例。两套 Rust 工具链的 8 个用例分别保留原 URL 及断言，对应 4 个数据库类型和 4 个资源 ID 案例。两个隔离错误变体分别触发了最后一个问号和 H2 类型的目标断言。 [当前验收记录](wp05-rm-jdbc-source.json)

其余 13 个案例涉及 JDK 服务发现及解析器身份、驱动实例化、普通及 XA 数据源初始化。17 条原 Java 初始化轨迹记录了连接关闭、注册时机和异常优先级。普通 JDBC 的关闭异常可成为 suppressed，XA 外层关闭可替换此前异常。Rust 初始化差分待完成。

RM JDBC 的解析器发现与单例两个原案例已匹配，当前累计 10/21。Rust 将 Druid 委托服务链接进程序，按 Locale 折叠名称、按有符号优先级选择并缓存单例。RM 以 ROOT 查找服务名，最高优先级并列会报错。完整 JVM 类加载隔离、进程默认 Locale 生命周期及卸载仍待完成。其余 11 个案例为驱动实例化和数据源初始化。 [验收记录](wp05-rm-jdbc-spi.json)

RM 已实现 `JdbcUtils::load_driver` 的宿主适配入口。上下文类缺失时回退到默认域，默认类缺失或构造阶段的指定异常包装成 SQL 异常，保留原异常对象及消息。17 条原 Java 轨迹与 Rust 适配边界逐条匹配。Rust 由宿主提供类句柄、构造器及 JVM 具体错误。原四个驱动加载案例及七个数据源初始化案例仍待迁移。 [适配边界验收记录](wp05-rm-jdbc-load-driver.json)

Rust 已接通普通 JDBC 与 XA 的数据源初始化宿主入口，依次写入资源字段、关闭连接并登记资源。18 条原 Java 轨迹验证初始化和清理顺序、异常优先级及注册条件，包括驱动类探测异常。探测失败后可重新探测。Java 形式的双失败保留 suppressed，Rust 专属错误和已识别的循环异常图通过双错误记录保留诊断。具体 JVM/数据库宿主、驱动加载四例和数据源初始化七例仍待迁移。 [适配边界验收记录](wp05-rm-jdbc-init.json)

驱动加载四个原用例已逐案匹配。测试 JVM 使用固定 JDK 和 H2 1.4.181，提供真实类查找、实例化和异常，Rust 正式入口负责回退及包装。固定原始 `JdbcUtilsTest` 的 21 项在原 classpath 上重新执行通过。真实 H2 实例、无效类名与空字符串的 SQLException 原因、以及 null 的 NullPointerException，均在两套 Rust 工具链匹配。缺失 H2 的负控触发原正例失败。RM JDBC 累计 14/21 个匹配，剩余七个普通 JDBC/XA 初始化用例。 [逐案记录](wp05-rm-jdbc-driver-jvm.json)

七个普通 JDBC/XA 初始化用例已全部逐案匹配。保留源 JUnit 的模拟连接及元数据输入，并使用固定原 H2 和 MySQL 驱动实例核对资源字段与注册。异常场景核对原 cause、空元数据后的关闭、空 XA 连接以及外层 XA 关闭抛错时内外连接各关闭一次。两套工具链的映射覆盖同一组 21 个原 `JdbcUtilsTest` ID。两模块累计 97 个原案例匹配。 [完整 JdbcUtilsTest 逐案记录](wp05-rm-jdbc-init-source.json)

## DataCompareUtils 原对象比较

Java 原对象入口接受可空记录引用、可空行列表和不完整行，使用原列名排序及原主键字符串拼接。Rust 以显式 Locale 和与物理镜像不同的输入类型提供原对象比较，保留左侧字段子集比较以及原 Map 键覆盖规则。该入口只返回原用例断言的布尔值或输入异常。MySQL 补偿使用既有完整镜像及原生主键元组契约，遵循已验证的一对一匹配。

两套工具链对原 `DataCompareUtilsTest` 和 `DataCompareUtilsBehaviorTest` 的 12 个具体用例、45 个断言逐值匹配。固定 Java 测试先原样执行，再由保留原断言的观察副本记录实际值。Fastjson 比较先核对原列名、JDBC 类型和空值，再按原顺序就地转换；所测日期时间用固定 JDK 原语提供等价表示。追加六项 Java 重复键与拼接碰撞观察由 Rust 原对象入口复现。原行比较入口按字段原值比较。完整 Java 值域与原诊断消息继续迁移。兼容入口应保留全空格 DATE 文本的数组越界异常及受支持小数主键的 BigDecimal 字符串格式。 [逐案记录](wp05-mysql-record-compare-source.json)

固定 JDK 17.0.12 已对照全空格 DATE 的异常类型及消息，Rust 在调用宿主前返回异常并保留两个原字段。五种 DECIMAL 主键输入覆盖普通格式和科学计数法的切换、负数与小数位数保留。新增六项属于补充差分，原 JUnit 分母仍为 12 个具体用例。 [边界修复证据](wp05-mysql-record-compare-boundaries.json)

行级兼容入口显式接收本次 Undo 序列化器名称。原名称 `fastjson` 逐字段调用现有原地转换，其他名称对当前受支持值使用原值比较。比较前构建双方原主键映射并保留重复键覆盖。按固定 JDK 的外层行和内层列的 HashMap 顺序比较左侧条目，首个缺行、缺字段、值差异或转换异常即停止，此前的变更继续保留在原输入。物理镜像走一对一比较。

五项行级补充观察已与固定 Java 逐值匹配，覆盖未设置序列化器、大小写差异、Fastjson 分派、重复行后的部分转换和原主键快照。显式名称由兼容入口的调用方提供。可变记录入口、生产 Undo 序列化器上下文、Jackson DmdbTimestamp、Java 对象共享引用和生产日期时间宿主继续迁移。 [行级验证](wp05-mysql-fastjson-row-compare.json)

可变记录入口先按原顺序检查记录引用、表名与行数。等价的空记录跳过元数据读取。非空记录采用左侧主键，按已验证的行级分派更新原字段。左表名和非空记录的左元数据缺失时保留 Java 的空引用异常类别。

原记录级入口已对照三组原 Java 观察。空行绕过元数据，缺失的左表名或左元数据映射为 NullPointerException，异常差分覆盖类别。非空记录使用左侧主键，经 Fastjson 转换后保留原值对象所在字段的更新。生产 Undo 上下文、Jackson DmdbTimestamp、共享 Java 对象和日期时间宿主继续迁移。 [记录级验证](wp05-mysql-fastjson-record-compare.json)

UTC JDBC 日期时间宿主以固定 JDK 17 的 Date、Time、Timestamp 解析及实际时间值为对照。在 UTC 用自纪元秒和纳秒表示同类型相等性，保留带进位时分秒、历法切换、分数精度和输入异常类别。由调用方明确选择 UTC，并使用同一规则构造已类型化的值。

原 Date、Time、Timestamp 的三个参数化用例已改用 Rust UTC 宿主。两套工具链对 24 项新原语观察逐值匹配，包括历法切换、负纪元、纳秒进位和 Unicode 数字。输入错误差分核对异常类别。非 UTC、LocalDateTime、NumberFormatException 消息及生产 Undo 接线继续迁移。 [UTC JDBC 验证](wp05-mysql-fastjson-jdbc-utc.json)

LocalDateTime 宿主采用固定 JDK ISO_LOCAL_DATE_TIME 的严格 ISO 日期、ASCII 数字与纳秒规则。接受 T/t、合法的扩展年份以及可选秒和小数，拒绝偏移量与时区。比较同一民用日期时间的年月日与纳秒，不引入时区转换。

原 Fastjson LocalDateTime 用例已改用 Rust 解析，固定 JDK 仍独立执行源用例及 29 项原语差分。有效输入比较全部本地日期时间字段与纳秒，无效输入核对 DateTimeParseException 类别。原异常消息、非 UTC JDBC 配置和生产 Fastjson Undo 解码与执行接入继续迁移。 [LocalDateTime 验证](wp05-mysql-fastjson-local-datetime.json)

Fastjson 默认内容逐案迁移覆盖 getDefaultContent 和分支信封解码。原字节为 {}，解码后保留存在的分支对象、空 XID、零分支号及空引用日志列表。JSON null、缺省日志集合和显式空日志集合分别可观察。只有通过分支身份和完整镜像预检的文档才可进入物理绑定。

固定 FastjsonUndoLogParserTest 的七个原用例已在隔离 H2 上重新执行。两套 Rust 对照继承用例 testDecodeDefaultContent 的默认载荷和四个原断言。该声明方法由六种序列化器执行，当前匹配 Fastjson 的单个具体用例。 [逐案验收](wp05-fastjson-undo-default-source.json)

Fastjson 原对象编码保留分支类型标记、可空表名和原字段的装箱类型。原 WriteClassName 用例按原对象构造输入，比较线缆和解码后的 Long、BigDecimal。未绑定的原对象仍须通过独立恢复预检，才能参与物理回滚。

固定版 testWriteClassName 的原对象构造、三个断言与完整 Java 编码均已对照。Fastjson 转义和溢出的 Long 标记由额外 Java 原语差分验证。普通原对象编码保留不一致的表名，物理预检拒绝该用例的恢复请求。 [类型保留逐案验收](wp05-fastjson-undo-write-class-source.json)

Fastjson SerialArray 迁移覆盖原测试所用有名 BIGINT 数组，保留 JDBC 基础类型、类型名、元素顺序和空元素。直接编码使用普通数值，嵌在原 Undo 时按 WriteClassName 写 Long 后缀。原对象数组可通过只读信封解码，MySQL 物理恢复继续校验既有列值域。

原 SerialArray 三个用例已逐字节对照，新增 12 项原断言全部匹配。直接编码和 Undo 内嵌编码保留原有差异。预检在可恢复 MySQL 镜像的替代字段中仍拒绝对象数组。当前对象范围为有名 BIGINT，其他基础类型和 Java 有损数值转换继续迁移。 [数组逐案验收](wp05-fastjson-undo-serial-array-source.json)

Fastjson 编解码与性能原用例保持固定 H2 的原 SQL、四份行镜像、两条 UPDATE 日志和 100000 次循环。Rust 以真实 JDBC 观测的原始字段构造日志，核对编码、对象中的表名/值及原断言；性能用例沿用原检查次数。

FastjsonUndoLogParserTest 的七个原用例均已通过 Java 和两套 Rust 对照。新增两个用例采用真实 H2 JDBC 查询结果及原用例的模拟 TableMeta，保留原 SQL、四份镜像和 keyType=NULL。性能用例执行十万次原编解码与分支号检查，耗时没有验收阈值。两个声明方法在其他序列化器下的实例继续跟踪。 [H2 编解码逐案验收](wp05-fastjson-undo-h2-source.json)

Jackson 继承用例使用原 H2 查询产生的镜像、默认内容和十万次循环。普通 Java 对象层保留 Jackson Bean 标签、集合、字段装箱类型及原值。比对原断言与完整 JSON 对象图，随后独立进行 MySQL 物理恢复校验。


固定版 JacksonUndoLogParserTest 的九个原用例已在隔离 H2 重新执行。两套 Rust 匹配默认内容、编解码和性能循环的三个具体用例，覆盖 18 个原断言位置及完整序列化字节。真实 JDBC 查询产生四份镜像，每次性能用例执行十万次。另用固定 Java 核对控制字符与补充字符转义，两套 Rust 均验证跨解析器重查输入字节预算。MySQL 恢复继续走独立的物理元数据绑定。 [Jackson 原用例验收](wp05-jackson-undo-h2-source.json)


Jackson SerialArray 迁移覆盖原 Field 编解码的两个 BIGINT 原用例。独立 Field API 保存类型名、元素顺序和空位，并核对原 JSON 成员顺序和整数类型。同一对象适配也用于普通 Undo 内的 Field。物理恢复按独立的列值域规则检查。


固定 Java 的两个 Jackson SerialArray 原用例和两套 Rust 已匹配 9 个原断言位置及完整 Field 字节。BIGINT 元素保持普通数值，空位保持 null。Rust 叶子读取严格检查数组成员和精确整数，物理恢复仍需既有列值域的独立校验。 [Jackson 数组逐案验收](wp05-jackson-undo-serial-array-source.json)


剩余四个 Jackson 原用例的 Java 观测已保存，Rust 迁移待完成。共有 15 条观察，涵盖独立 Field 的 Long、BigDecimal、Double、LOB、普通及达梦时间戳，以及继承用例内的空 SQLType、空镜像类标签和纳秒时间戳。对照时同时检查 Java 类标签、时间值与原断言结果，保留达梦原始无时区值与反序列化值在普通比较和 Jackson 模式下的不同结果。 [固定 Java 观测](evidence/fastjson-undo-native/seata_jackson_timestamp_trace_2fllzl5_/next-java-observations.tsv)


Jackson 独立 Field 的 encode 原用例已与两套 Rust 匹配。九个断言对应的 Java 类和完整字段字节逐项一致。Timestamp 和 UTC LocalDateTime 分别按原解码规则恢复毫秒和纳秒。固定 Java 边界观察另验证了空 SerialBlob 只留下类标签，Rust 编码返回错误，解码接受带空 Base64 值的合法对象。有限 Double 的正负零可区分，科学计数法的 JDK 文本格式继续跟踪。 [独立 Field 原用例验收](wp05-jackson-undo-field-source.json)


达梦时间戳的补充 Java 观测记录了 dtype、scale、时区标记和各内部时间分量。原用例的 UTC 无时区源值往返后变成带时区的值，通用对象比较为 false，同一瞬间的比较为 true。不同毫秒值仍须比较为 false。UTC 与东八区附加观测保留在证据中，Rust 两个达梦原用例尚待绑定。 [达梦驱动状态观测](evidence/fastjson-undo-native/seata_jackson_dmdb_kqi3emy8/dmdb-state.tsv)


两个 Jackson 达梦时间戳原用例已在固定 Java 和两套 Rust 上匹配五个原断言及完整字节。UTC 对象投影保留 Date 与 ZonedDateTime 的来源差异，瞬间比较保留亚毫秒位。当前接口从 JDK 公历切换点起支持 UTC，切换前历法和其他宿主时区继续跟踪。 [达梦逐案验收](wp05-jackson-undo-dmdb-source.json)


继承的 Timestamp 用例需要在普通 Jackson 分支对象中保留 null SQLType、null 表名及 EmptyTableRecords 标签。Rust 从原构造值独立生成带纳秒的 BranchUndoLog，核对完整字节和恢复值，并继续通过物理绑定校验恢复资格。


Jackson 继承的 Timestamp 原用例已匹配。两套 Rust 从原构造值生成完整分支字节，恢复 2147483000 毫秒及 999999 纳秒。伪造含行的 EmptyTableRecords 在对象投影时被拒绝，原构造日志也因缺少物理表名被恢复入口拒绝。Jackson 原测试类累计九例全匹配；同一 Timestamp 方法在 Kryo 和 Protostuff 下的两例仍待迁移。 [完整 Jackson 原类验收](wp05-jackson-undo-branch-timestamp-source.json)

固定 Java 的 Fastjson2 原类五例已捕获。原默认内容为 71 字节 JSONB，H2 编解码与十万次循环的载荷均为 650 字节，两个数组用例分别为 457 与 461 字节。字节以十六进制原样记录，状态为 JAVA_CAPTURED，Rust 待逐案绑定。 [Fastjson2 原 Java 观测](evidence/fastjson-undo-native/seata_fastjson2_junit_m421pkwd/java-capture.json)


Fastjson2 现已通过五个原 JUnit 的双工具链对照。Rust 独立生成并读取 JSONB，保持类型标签、字段符号表、装箱整数、数组空位及空镜像标签。覆盖 27 个原断言位置、真实 H2 镜像、每套工具链十万次循环，并对照了四个固定 Java 字符编码样本。引用词法、其他未覆盖的 JSONB 值和 MySQL 恢复接入继续跟踪。 [Fastjson2 原类验收](wp05-fastjson2-undo-source.json)

Protostuff 原测试类四个用例已保存 Java 二进制观测。默认内容为十六进制 1000。原 H2 及纳秒时间戳均保留完整载荷，Rust 待逐案绑定。 [Protostuff 原 Java 观测](evidence/fastjson-undo-native/seata_protostuff_junit_5u6c6vfx/java-capture.json)


Protostuff 原测试类四例已通过固定 Java 和两套 Rust 对照。原分支的分组顺序、装箱字段、有符号变长整数、真实 H2 镜像及 Timestamp 委托的 12 字节载荷逐字节匹配。Java 补充探针确认九个标量样本、NaN 原始位和 Timestamp 委托顺序。原 Timestamp 方法在 Kryo 下的剩余实例继续跟踪。 [Protostuff 原类验收](wp05-protostuff-undo-source.json)

Kryo 的四个原 Java 用例已捕获。观察进程只打开固定 Kryo 序列化器需要读取的 java.util 和 java.text 模块，原断言全部执行，Rust 尚待绑定。 [Kryo 原 Java 观测](evidence/fastjson-undo-native/seata_kryo_junit_bizk14gs/java-capture.json)


固定 Kryo 工厂的 43 个类注册项已由 Java 运行时直接记录。相关编号包括 BranchUndoLog 42、ArrayList 22 和 Timestamp 11。Rust 编解码需依据固定的类编号、动态类名和引用语义对照四个源用例。 [Kryo 类注册观测](evidence/fastjson-undo-native/seata_kryo_registry_gyxh3d5f/registrations.tsv)


Kryo 原测试类四例已在固定 Java 和两套 Rust 上匹配。对照覆盖原类编号、动态类名、可空枚举、字符串引用、真实 H2 镜像、十万次循环及纳秒时间戳。Rust 对等值字符串复用引用，其他对象别名和完整 Java 对象图仍待迁移。继承 Timestamp 用例的 Jackson、Protostuff 和 Kryo 三个实例均已匹配。 [Kryo 原类验收](wp05-kryo-undo-source.json) · [Kryo 5.4.0 引用规则](https://github.com/EsotericSoftware/kryo/blob/kryo-parent-5.4.0/src/com/esotericsoftware/kryo/util/MapReferenceResolver.java)

Fory 三个原用例已在固定 Java 执行并记录原二进制。原声明的默认内容、H2 编解码及性能循环各有六种序列化器实例，Fory 待完成 Rust 绑定。 [Fory 原 Java 观测](evidence/fastjson-undo-native/seata_fory_junit_vhv5fllo/java-capture.json)


Fory 三个原用例已在固定 Java 和两套 Rust 上匹配。类描述由固定 JVM 对六个类型独立生成，Rust 按完整描述匹配固定格式并校验正文偏移。真实 H2 镜像、标量装箱与引用、默认内容以及十万次循环均保留原载荷。多态等值文本、布尔值和固定 Java 装箱缓存范围内的整数可生成引用；读取端另验证了 Java 共享 Double。任意对象身份、其他类描述及 MySQL 恢复接入继续跟踪。 [Fory 验收](wp05-fory-undo-source.json) · [Fory 0.12.3 Java 格式](https://github.com/apache/fory/blob/v0.12.3/docs/specification/java_serialization_spec.md)

固定目录中六个 UndoLogParser 原测试类共 32 个具体用例均已匹配。继承的默认内容、编解码、性能及 Timestamp 四个方法均已覆盖其全部原实例。此结果只对应原目录中的具体用例，完整对象语义与 WP05／WP06 的物理恢复和故障门槛继续开放。

## Undo 解析器工厂、SPI 与分支对象

固定 Java 的 UndoLogParserFactoryTest、UndoLogParserProviderTest 和 BranchUndoLogTest 均已执行。Rust 工厂在默认配置下选择 Jackson，具名入口支持 `fury` 别名并拒绝已移除的 FST。SPI 使用 inventory 登记六种已有解析器，工厂和 SPI 共享同一解析器实例。名称比较保留固定 Java 的大小写行为，默认配置覆盖由调用方通过具名入口传入。MySQL 锁定根行继续按原上下文精确匹配。

原 BranchUndoLog 用例经工厂取得 Jackson，使用原 XID、分支号、可空日志表名及前后镜像，逐字节匹配 Java JSON 并保留原分支号断言。补充观测覆盖 36 条名称、异常与实例共享结果。 [原用例验证记录](wp05-undo-parser-selection-source.json)

## TableRecords 原对象与结果集投影

固定 Java 的 TableRecordsTest 六个原用例均已重新执行。Rust 的原记录对象复用 sqlparser 的 TableMeta，临时元数据仅能附加一次，空镜像拒绝追加及读取元数据。结果集投影使用缓存列的 JDBC 类型和列名，保留结果列与行的顺序，并拒绝缓存中不存在的列。Druid mock 的专用 getter 返回空 Blob 与 Clob，Rust 按相同装箱语义保留空值对象。主键对照记录并核验每行键数和值。

前五例的结果集投影已经与固定 Java 匹配。第六例保留 JDBC 2014 的 OffsetDateTime 装箱类型，经默认 UTC Jackson 数值模式逐字节匹配并恢复原值。补充差分涵盖负秒与负亚秒、纳秒精度、偏移转 UTC、日期边界、非法小数尺度及带类型标签的 Undo 字段。此阶段输入为已读取的 JDBC 值，使用兼容对象层。真实数据库驱动和物理回滚仍按既有验收契约执行。 [前五例证据](wp05-table-records-source.json) [第六例及类级验收](wp05-table-records-offset-source.json)

## RM 元数据四类原测试的补充迁移

本轮选择 rm-datasource 中 ColumnMetaTest、IndexMetaTest、IndexTypeTest、TableMetaTest 四个完整原类。旧完整 JUnit 基线的物理目录缺失，不据历史清单声称已恢复全部基线。新运行先核对后续封存 classpath 清单的摘要，只读取字节完全匹配的缓存输入；目标生产类与原测试类由固定 Git blob 在 D 盘重新编译，由原 JUnit 引擎重新执行。实际加载来源与所需依赖另行封存并重放。

用于实参差分的观察副本与未改动的原测试严格分开：仅在副本中把 Assertions 调用替换为记录器，再委托未改动的原 JUnit。保留十个原案例的全部断言、顺序、异常和对象变更；Rust 使用已正式实现的元数据类型，逐案例比较记录。原案例完成与全部原对象/JDK 环境完成分开统计，不推造缺失的全局基线总数。原始测试、资源和 Java oracle 不修改。

本轮十例及 51 处原断言在固定 Java 和双 Rust 已实际匹配，四个 Rust 测试文件分别对应原类；十三项已验证依赖通过 portable 模式独立重放。程序只读复核通过，但未进行独立人工或另一 Agent 审查，因此该审查门禁与完整 WP05 仍开放。旧完整基线的不可用状态和历史总数已显式标注，不将历史计数机械累加为当前事实。[元数据增量证据](evidence/rm-metadata-source/README.md) · [逐案账本](wp05-rm-metadata-source.json)
