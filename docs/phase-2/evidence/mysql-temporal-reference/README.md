# 时间类型：原 Java 实际保真参考

对应 [WP05 时间类型契约](../../mysql-temporal-compatibility-contract.md)。本现场用于确定正式实施的真实差异，不声称 Rust 支持或原 Java 全部时间类型通过。

权威运行：[java_20261128_093237_32f6b742](java_20261128_093237_32f6b742/result.json)，catalog SHA-256：`e4ee541b284403d5c9a3b98d15b25f59181ff31e698645ab50454ca7aa453476`。18 个固定原源码重新编译；依赖来自已封存历史包，未声称执行完整 Maven 重建。原类及驱动实际加载来源、编译输入、原始 Undo、数据库绑定后值、命令及容器清理均保留。

| 每组观测结果 | 次数 | 含义 |
|---|---:|---|
| 合法数据库行 | 29 | DATE 13、TIME 10、TIMESTAMP 6；均含 SQL NULL |
| 原驱动读取异常 | 7 | DATE 历法缺口 3；TIME 负时长/超日范围 4 |
| 重绑定后物理值改变 | 4 | TIME 微小负值、微秒、毫秒的丢失 |
| 原编码/解码 Java 值改变 | 2 | TIME 毫秒被字符串序列化去除 |

四组为 JVM UTC/Asia/Shanghai × 驱动及数据库会话 UTC/+08:00，共 116 次观测，不是 116 笔全局事务。每组 TIMESTAMP 六行完整重绑定，保留微秒、纪元和 SQL NULL；这不是全范围、正式回调或 Rust 端认证。DATE 的 1582-10-05、10、14 已由 MySQL 接受，原 JDBC 读取失败。TIME 的 `-00:00:00.000001`、`00:00:00.000001`、`12:34:56.123456`、`23:59:59.999999` 重绑定后都失去相应精度。

独立验证器直接核对每个物理值、原 Java 类/JDBC 编号、纪元、纳秒、原 Undo、重绑定结果和失败阶段；不能通过改写摘要掩盖反例。验证命令：

```powershell
python -B tools/formal-source-junit/validate_temporal_reference.py `
  docs/phase-2/evidence/mysql-temporal-reference/java_20261128_093237_32f6b742 `
  e4ee541b284403d5c9a3b98d15b25f59181ff31e698645ab50454ca7aa453476 --weakenings
```

初次探索 [java_20261128_092933_975ec997](java_20261128_092933_975ec997/result.json) 使用 Java `List.of` 作为日志集合，实际类型是 `ImmutableCollections$List12`；随后入口改为原常见 `ArrayList` 并完整重跑，以上权威运行采用后者。初次探索保留作诊断，不能替代最终输入绑定。两个运行的专属 MySQL 容器均已移除，停止后的数据保留在其 `cleanup.json` 记录的 D 盘目录，没有删除历史缓存。

下一步按契约实现并验证 TIMESTAMP，再解决 DATE 日历和 TIME 表达差异。完整范围没有缩减，未把这些反例标成“兼容通过”。
