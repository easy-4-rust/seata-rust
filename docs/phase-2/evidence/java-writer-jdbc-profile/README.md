# Java 写入器完整 JDBC 配置准入

权威探针包：[20261128_124805_b9ba7380](20261128_124805_b9ba7380/result.json)。目录 catalog SHA-256：`59b408719803b44b61a0cceed9f43444a2a1de70e1c6386a6ed7fbd25a658799`。

测试使用固定 MySQL 8.4.13 镜像、真实 Connector/J 8.0.27 和原 Seata `ConnectionProxy`。仅 `DataSourceProxy` 的资源 ID 与实际数据源引用由 Mockito 提供；未创建或验证 TC 全局事务，原 SDK 初始化产生的后台连接日志不计为 TC 验证。

- 六种实际表示各尝试六种部署绑定：6 接受、30 拒绝。
- 另外 39 次拒绝覆盖声明与实际表示不一致、非法码、NULL、session/JVM/parser/驱动时区及缺少或多用精确装饰器。新列缺失的原生 1054 另行核验，不计入 69 次 SQLState 25000 拒绝。
- 每种匹配配置都在原事务保持绑定共享锁：独立部署连接 UPDATE 实际返回 1205；原事务回滚后同一连接可更新一行，共六组。
- 原三参数入口在 NULL 和缺少 021 新列时通过；切换真实 8.0.33 驱动后仍通过，新四参数入口拒绝该驱动。原 Java actor 与新守卫在不加入精确装饰器类的 classpath 中编译通过。

`source/` 保存编译源码、运行器、迁移及旧 actor；`classes/` 保存本次编译的类，`dependencies.json` 保存实际依赖哈希，两个 class-load 日志标明守卫、原代理、驱动及 parser 来源。独立校验器检查 catalog、当前源码、实际运行文件、执行退出码、观测计数及类来源。

```powershell
python -B tools/formal-source-junit/java_writer_jdbc_profile.py
python -B tools/formal-source-junit/validate_java_writer_jdbc_profile.py docs/phase-2/evidence/java-writer-jdbc-profile/20261128_124805_b9ba7380
```

`20261128_124336_f1cbdd05` 保存新增重载尚不存在时的预期编译失败。更早两次是诊断语言适配的失败尝试；`124418` 是缺少原 SDK Spring 环境配置的失败尝试，`124545` 是较早通过但尚未加入来源日志的探针，`124705` 是缺少旧 actor Lombok 编译依赖的失败尝试。均保留原始结果，不作为最终验收。

此证据不关闭完整 Java RM 时间类型补偿、所有写入器强制接入、数据库权限和 WP05 总门禁。
