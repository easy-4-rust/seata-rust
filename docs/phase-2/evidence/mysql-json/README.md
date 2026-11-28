# MySQL JSON 列：原 Java 格式与正式 AT（选定配置已验证）

对应[实施契约](../../mysql-json-contract.md)、原架构 9.3 和 WP05。正式 MySQL AT 现支持非主键 JSON 列的元数据、前后镜像、原 Java Undo 和补偿。新增独立 `ColumnDomain::Json` / `MySqlType::Json`，Rust 值保持 `FieldValue::Text`；SQL NULL 使用 `FieldValue::Null`，JSON null 使用 `FieldValue::Text("null")`。JSON 参数继续经原 SQLx 绑定，不扩大 SQL 语法白名单。

```mermaid
flowchart LR
    DB[原生 MySQL JSON 列] --> M[保留 JSON 物理身份，无字符长度声明]
    M --> T[逐行核对原生 JSON 类型]
    T --> V[原 UTF-8 文本 / 独立 SQL NULL]
    V --> J[原 JDBC -1 / java.lang.String]
    J --> U[Java Jackson Undo + 原事务 SHA-256]
    U --> C[原 TC 同 BranchId 提交或回滚]
    C --> O[原业务行 / Undo / 终态 / Completed]
```

固定原 Java commit `cba73e180fd8896a99df6415b859d275122feaa7` 的 18 个选定源码与独立 JSON 对照类重新编译；依赖来自封存且哈希核验的原 Jackson 2.18.3、Connector/J 8.0.27 与辅助类，不下载/升级环境，也不称为完整 Java Maven 模块重建。原 `TableRecords`、`JacksonUndoLogParser` 和对照类的实际加载来源均为这次编译目录。原 TC JAR 与只用于本地登记的认证覆盖保持固定原边界。

原 JDBC 现场确认 `JSON` 的类型码为 `-1`、值类为 `java.lang.String`，MySQL 的字符集/字符长度元数据为 NULL。不能用 LONGTEXT 元数据替代。Rust 只在核对实际 MySQL 值类型为 JSON 后取原文本字节，保持其他类型的原 SQLx 检查；不通过 JSON 数字模型二次解析。保存的是[服务器 JSON 文本语义](https://dev.mysql.com/doc/refman/8.4/en/json.html)，输入的空格、键顺序和服务器自身数值规范化按原服务器处理。

双 Rust 1.99/1.85 分别执行原 TC Commit/Rollback，各九行基线涵盖对象、数组、空集合、字符串、布尔、SQL NULL、JSON null、补充 Unicode、9007199254740993 和 18446744073709551615。Rust 三类镜像的完整 Undo 对象图与原 Java 输出逐字段一致，JSON 内部文本精确一致。每笔原全局事务执行实际 UPDATE、INSERT、DELETE，再按原分支完成一次报告和指定结束；独立 SQL、原关联帧、持久见证/摘要、Undo 删除或围栏、原 TC 分支成功日志和 Completed CRC 联合核对。

回滚方向另以受控外部写入制造 JSON 脏镜像，直接调用正式资源接口得到 `ManualInterventionRequired`，目标脏值及原 Normal Undo 保持。还原原 after 文本后，由原 TC 实际回调完成同一分支。该部分为正式资源接口的数据库探针；提交/回滚结果另由原 TC 真实报文证明。

每个原 TC 场景还验证非法 JSON 的 MySQL 3140、随后业务调用 Poisoned、错误行未写入，以及超过 1 MiB 的有效 JSON 在镜像读取前被预算拒绝。默认工作区增加两项精确值/Undo 测试与一项 SQLx 原始协议测试；后者覆盖原生 JSON、SQL NULL、伪装 VARCHAR（含 NULL）和非法 UTF-8 五种行。移除原生类型核对、错用 JDBC 12、移除字段预算，两个工具链各三项源码反例均命中实际目标断言。源文件全部恢复并与最终运行绑定。

最终四笔 JSON 原事务通过，62 项 JSON 证据削弱拒绝。共享代码的双工具链普通 AT 四并发各八轮另执行 64 笔回归，40 项证据削弱拒绝。全工作区各 804 passed / 397 ignored、182 个摘要，MySQL、AT、数据源和可观测性全目标全功能严格 Clippy 通过；JSON 与普通 AT 的 ignored 现场函数由运行器另行显式执行，不把忽略计为通过。

修复前原实现明确返回 `Unsupported { feature: "column type" }`；早期夹具同时记录了任务失败后的外层屏障超时，现已改为工作任务失败立即退出。依赖恢复、代理启动参数及大字段表清理的早期诊断原样保留；权威仅为[最终核验](validation_20261128_final)及[机器 ledger](../../wp05-mysql-json.json)绑定的源码、四笔 JSON、64 笔普通 AT 与双版本回归。

JSON 主键、JSON 函数/路径更新等语法未因此放开，其余列沿用现有类型范围。无需新增恢复表迁移或依赖，旧格式和其他类型规则保留，有变化 COMMIT 仍需显式实验特性。完整 WP05、生产准入和全部后续阶段仍开放。
