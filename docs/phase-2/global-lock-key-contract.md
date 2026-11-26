# WP05 原主键锁键与事务锁集合

延续[WP05实施契约](wp05-mysql-at-plan.md)，补齐原Java `BaseTransactionalExecutor#buildLockKey/prepareUndoLog` 和 `ConnectionContext#appendLockKey/buildLockKeys` 的必需路径。固定Java pin不变；不改变分支登记、同连接Undo、提交未知围栏及原WP05/06验收。

## 对象与可观察行为

`seata_at/exec/base_transactional_executor.rs`实现上述对象的锁键方法，保持一个Java对象对应一个真实文件；`connection_context.rs`只实现有界锁集合，原保存点、XID/分支上下文等仍PARTIAL。

- 单镜像空行返回None；非空输出`table:pk1_pk2,pk1_pk2`。表名保留原元数据，行序保留数据库镜像顺序。读取/SQL绑定仍用真实主键索引序，但锁键必须遵循原`TableMeta#getPrimaryKeyOnlyName`实际返回序：该pin把PRIMARY索引列逐个插入默认Java HashMap再遍历。Rust需单独复现固定JDK17的此有界次序（含resize及碰撞树），不能直接以索引序、Rust HashMap或字符串排序替代。两种次序独立保存；否则同一复合PK可能获得不同全局锁键。
- DELETE从before取锁，INSERT/UPDATE从after取锁；两侧均空不追加。每条语句的完整锁键作集合去重，不能把不同行序/不同分组自行合并成另一种字节。事务输出分号分隔；Java HashSet无顺序保证，Rust使用稳定完整键排序，对照以完整键集合及原逐语句字节为准，不把此排序当行序排序。
- 原生整数转精确十进制，二进制按原Seata ArrayUtils转Java有符号byte列表（含括号/逗号空格），DECIMAL按BigDecimal.toString保留scale及科学计数法边界，不能经f64。signed tinyint(1)按明确的已测JDBC profile保持Boolean或numeric；禁止默认profile静默把物理2改成true。原生身份缺失拒绝猜测。
- 保留原validPk对文本逗号的拒绝；不自行转义、hash或重新定义TC协议。原Java允许的空白、分号和二进制逗号需要独立核对TC实际拆分及锁覆盖，纯格式互通不能证明这些身份可安全登记。提交路径启用前必须解决锁覆盖差异或取得已要求的核心决策，不以新格式回避原Java。
- 单键/事务总输出各最多1MiB；128条准备日志；预算在扩展String之前检查。追加失败不改原集合；Debug/错误不输出主键或原值。
- 同一事务锁集合固定一个明确JDBC profile，即使当前字段字节相同也不能混入另一profile；从空日志建立的显式上下文仍固定该配置。普通new/with_limit首次成功追加才绑定，失败不改变配置或计数。

## 验证与集成顺序

先真实编译/调用原buildLockKey及ConnectionContext，保持原四组锁键测试输入和断言，再补原主键索引序、整数/DECIMAL/Boolean/二进制及集合/预算的Rust义务。原对象整体和完整源案例仍不能因方法探针通过而升格完成。

之后把实际MySQL准备镜像交给同一生成器，并与原JDBC读取及原方法比较；最后接真实BranchRegister和Undo落库/恢复。调用方只能从原TC成功响应取得分支号；本组件不发送登记请求、不写Undo、不提供COMMIT。原共享锁问题继续按用户要求暂缓，本增量仅涉及AT排他锁键。
