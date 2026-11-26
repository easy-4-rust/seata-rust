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
