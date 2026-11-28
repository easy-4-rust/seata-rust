# 精确 DATE/TIME JDBC 适配器（组件阶段）

这是显式选择的 Connector/J 8.0.27 缓冲结果集装饰器，已验证 JDK 17.0.12、MySQL 8.4.13，覆盖客户端和服务器预编译语句。Rust 显式配置码 5/6 已接入正式 SQL/Undo，并有原 Java TC 驱动正式 Rust RM 的现场；Java RM 全事务路径与完整时间类型验收仍未完成。

```java
DataSource actual = /* 现有 Connector/J 数据源或连接池 */;
DataSource exact = new ExactTemporalDataSource(actual);
DataSource seata = new DataSourceProxy(exact);
```

必须把装饰器放在 Seata `DataSourceProxy` 内层。泛型 `ResultSet.getObject` 对 DATE/TIME 返回精确字符串或 SQL NULL；JDBC 编号仍为 91/92。原 Seata Jackson Undo 对象图和 parser 不变，但显式模式中的字段值类变为 String。它不承诺原 `java.sql.Date/Time` 字段表现，也不自动转换旧分支。原默认模式、Rust 配置编号 1–4 及旧 Undo 记录没有修改。新增 5/6 为显式精确模式，保留对应 Boolean/数字 TINYINT 和 UTC 约定，详见 [正式路径现场](../../docs/phase-2/evidence/mysql-exact_date_time/README.md)。

选择此适配器前须统一同一资源的全部写入器，并先排空原表示下的未完成分支。仅认证原 Jackson 路径；Fastjson 的原比较器会把字符串转换成 `Date/Time`，尚未验收。SQL 拦截、锁键、原子 Undo 与正式回调已取得选定现场，专用跨进程改写拒绝/原字节恢复、DATE 普通列和 TIME 主键已取得 [原 TC 现场](../../docs/phase-2/evidence/mysql-exact-temporal-restart/README.md)；原类型专门现场回归与写入器准入等门禁仍开放。

读取使用原始行字节与真实字段 FSP，避免 JDK 1582 年日历切换、负一微秒符号及大时长损失；类型化字符串写入使用 `setString`，避免客户端预编译的 TIME 文本重解析。非 DATE/TIME 及显式类型 getter 保持委托。DATE 认证范围 1000–9999 年，TIME 为 ±838:59:59.000000，0–6 位精度不匹配时拒绝而不舍入。

尚未认证流式/动态结果集、其他驱动版本、CallableStatement 命名参数和 OUT 参数、厂商 unwrap 后的直接调用及所有第三方连接池。厂商 unwrap 可访问原对象，调用方须维持相同入口约定。认证证据见 [组件验收](../../docs/phase-2/evidence/mysql-exact-temporal-adapter/README.md)。

复现入口在仓库 `tools/formal-source-junit/`：`exact_temporal_reference.py`（真实服务器矩阵）、`validate_exact_temporal_reference.py`（独立核验）、`exact_temporal_reader_checks.py`（原始行解码边界）。使用已有 JDK 与封存依赖，不安装组件、不修改原 Java 核心。
