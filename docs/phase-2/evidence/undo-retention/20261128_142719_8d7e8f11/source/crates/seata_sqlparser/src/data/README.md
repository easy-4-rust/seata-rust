# 固定 JDK 17 字符映射输入

`java17_character_case.bin` 由 `tools/formal-sqlparser-core/MetadataCaseOracle.java`
执行固定 JDK 17.0.12 的公开 `Character.toUpperCase(int)` / `toLowerCase(int)` 生成。
每条记录为三个大端 u32：原码点、简单大写码点、简单小写码点；共 2799 条，按原码点递增。
未出现在表中的码点保持原值。映射数据属于生产兼容性输入，不属于迁移后的原 Java 测试资产，
也不计为计算结果 MATCH。

该数据只服务于原 `String.CASE_INSENSITIVE_ORDER` 比较器。
`String.toLowerCase()` / `toUpperCase()` 的多码点展开、上下文和默认 Locale 是不同契约，
TableMeta 的大小写索引映射及 containsPK 仍需单独迁移，不能用此表宣称已完成。

原 JDK 比较器在含孤立代理码元的字符串间可能不传递。
因此主键集合保留原 TreeMap 插入、旋转、根路径查询和中序遍历，禁止换成排序数组或宿主 Rust 排序集合。

## Locale 上下文依赖输入

`jdk17_string_casing_input.bin` 保存原JDK17.0.12实际单码点ROOT字符串转换、组合类和cased属性，
共5004条，按码点递增；每条依次为大端u32码点、u8组合类、u8布尔属性，
再跟两组u16长度及相应大端UTF16小写/大写码元。这里的ROOT结果包含多码点展开。

`jdk17_word_root_input.bin`、`jdk17_word_th_input.bin` 保存原词边界状态机、回退表、
接受/前瞻标志和压缩字符分类；`jdk17_word_th_dictionary_input.bin`保存原泰语词典状态表。
导出器为`MetadataLocaleOracle.java`和`MetadataLocaleProbe.java`，输入格式由实际大端输出定义。
原补充平面分类表允许同一起点的空区间，不能误拒；不能用当前Rust/Unicode分词表替代。

以上四份数据均为版本化兼容性输入，不能计为计算结果MATCH或原Java测试资产。
`JdkCasing`接受`Locale#getLanguage`规范化语言值，完成显式上下文转换。
默认系统Locale的读取、进程默认Locale管理以及TableMeta映射集成仍是后续工作。
