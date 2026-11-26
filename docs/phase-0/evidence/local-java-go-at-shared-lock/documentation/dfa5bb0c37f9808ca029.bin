# GO-AT-RESOURCE-ID-001：资源标识与凭据边界（OPEN）

固定Go提交 `82570d905919c1c66bb025a3a5979f25390de498` 的 `pkg/datasource/sql/driver.go#parseResourceID` 只截掉问号后的参数并替换逗号，保留原DSN前缀。真实AT运行的103注册、11分支注册、3/5回调及SDK日志均包含测试DSN的用户名和密码。本轮使用专有临时容器的固定测试凭据，没有连接或操作外部业务数据库。

固定Java提交 `cba73e180fd8896a99df6415b859d275122feaa7` 的 `MysqlResourceIdInitializer#initDefaultResourceId` 从JDBC URL截去查询参数。常见 `jdbc:mysql://host:port/database?...` 与Go的 `<user>:<password>@tcp(host:port)/database?...` 不会得到相同资源标识。

实际Go锁键为 `ACCOUNTS:1;`，该测试如实保留大小写与分号，不把它规范化后冒充Java相等。跨语言锁键和资源命名空间需要另行对照，不能由单Go RM正常提交/补偿推断共享锁安全。

影响：凭据可能进入TC资源关联/会话记录和日志；不同凭据或语言生成不同标识时，同一物理数据库可能使用不同锁命名空间。后者是基于源码/实际格式差异的风险推断，尚未执行Java RM与Go RM并发争锁证明。尚未对原Go或候选引入静默标识迁移。

后续应对照Java的URL、查询参数、大小写、别名、IPv6、负载均衡和显式资源覆盖规则，制定不包含凭据的稳定标识；同时验证旧持久Undo/资源缓存及跨语言互操作。正式Rust不能直接复制该Go DSN截断函数。安全日志与迁移策略需要独立验收。

[实际AT证据](evidence/local-go-java-real-at/README.md)、[实施范围](go-java-v1-real-at-plan.md)。本项和完整WP-00保持开放。


## 真实跨语言共享行验证（2026-11-13）

此前“风险推断、尚未争锁”是初始审计状态；[070224真实证据](evidence/local-java-go-at-shared-lock/README.md)已按[计划](java-go-at-shared-lock-reference-plan.md)复现：原Java DataSourceProxy与原Go seata-at-mysql在同一MySQL行先后完成本地阶段一，同时保留各自Begin/分支/非空Undo；余额1000→900→800。固定Java实际锁键仅切换Go resource后原锁不可见，独立证明不同资源标识隔离全局锁命名空间。固定Java资源换实际Go锁键也可锁，键文本不一致仍是独立兼容项；未分开归因大小写与尾分号各自的影响。

原Java客户端实际默认V2、Go为V1，透明代理原样转发并分别严格解析；不冒充Java V1 RM验证或Rust V2能力。12原生查询72帧、16类来源、6关键源/类原pin验证、118运行/32追加输入、342classpath/2802依赖文件，以及32 Python门禁均通过独立复审。原TM正常提交只负责清理：最终余额800/Undo空且锁释放，不认证安全互斥、Undo互换或资源迁移。

GO-AT-RESOURCE-ID-001仍OPEN，现从格式风险推断升级为所选真实跨语言组合的实际锁绕过。下一步须实现显式稳定无凭据资源与Java等价锁键方案，并重跑相同物理行的冲突拒绝、提交/补偿、重连与持久恢复；不能仅修资源字符串而忽略锁键或旧Undo/缓存。
