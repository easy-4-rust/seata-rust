# Java / Go AT 同一行共享锁：真实缺陷调查

按[现有增量计划](../../java-go-at-shared-lock-reference-plan.md)，正式负向主运行是 `run_20261113_070224_d7adfec7`。它证明GO-AT-RESOURCE-ID-001的实际风险，不是安全互操作通过，也不是修复或正式Rust组件。

固定Java DataSourceProxy真实AT先把同一accounts主键行1000改为900，本地提交并持有原全局Begin/分支/Undo/TC锁；固定Go SDK随后原Begin、真实seata-at-mysql把同一行900改为800。独立SQL同时读取两个原XID/BranchId及各自非空原Undo，两个全局事务均仍为Begin。没有修改线上resource/lock字段或合成RM成功。

| 观察阶段 | Java资源＋Java锁键 | Go资源＋Java锁键 | Java资源＋Go锁键 | Go资源＋Go锁键 |
|---|---|---|---|---|
| 仅Java阶段一提交，Go已Begin但未执行SQL | 不可锁 | 可锁 | 可锁 | 可锁 |
| 两个阶段一均已提交，全局均Begin | 不可锁 | 可锁 | 可锁 | 不可锁 |
| 两个原TM提交、AT清Undo及TC终态后 | 可锁 | 可锁 | 可锁 | 可锁 |

每格均为原生只读101/15/21、独立锁查询身份及原TC响应，不借同XID重入。固定Java锁键仅换resource，原锁即不可见，独立证明资源命名空间差异；固定Java资源换为Go实际锁键也可锁，说明键文本差异另需兼容。此实验不分开证明大写与尾分号各自的作用，不把全部URL/大小写/别名问题一并宣布解决。

实际Java资源为jdbc:mysql URL去查询参数；实际Go资源含测试DSN凭据。Java锁键`accounts:1`，Go为`ACCOUNTS:1;`。证据仅使用自有容器固定测试凭据，不接触外部数据库。最终通过各自原TM正常提交清理，余额800/Undo0/原事务缺席且锁释放；不交叉回滚，不认证两种Undo格式互换、dirty image补偿或旧标识迁移。

原Java客户端默认发送V2，Go发送V1；透明代理保留原版本和全部原始字节，分别严格解析102/104格式，没有把Java V2帧改写后冒充V1认证。原TC仍为同一固定JAR，仅加载已认证10class前导候选；实际FILE锁管理/ATCore/SessionHolder均从原JAR加载。本场景不扩大Rust的V2产品边界。

1个TC JVM、1个真实Java客户端JVM、1个Go SDK；118运行输入、342classpath/2802依赖文件与原JAR/SDK候选/actor二进制前后核对。16实际类来源、6个关键RM/TM源逐LF等于原pin和D构建副本、对应6class等于原参考target字节；Java actor编译产物结束复核。Java12帧/Go48帧及12只读查询72帧独立重放。

本轮32 Python=12归一化＋20实际证据破坏检查。12的RED11失败/1控制只是未启用guard适配器控制；20的RED18失败/2控制来自原冻结runtime校验器，补强后全部通过，证明新增Begin/Commit/报告/类来源/SQL门禁有效。两种RED均不是原Java生产单元测试失败。Go254/Rust117单元父范围继承，未声称本轮重跑。

最终认证必须同时通过原运行与追加门禁；32个追加输入独立冻结，不篡改118输入的原epoch。原实现缺陷仍OPEN，安全共享锁/稳定无凭据标识/锁键兼容/资源迁移/正式Rust与完整WP00未完成。自有进程与容器停止，28091/28092/28093/28094四端口释放。早期空classpath、缺Driver/Lombok及错误V1-only代理失败保留为工具诊断，均不算缺陷复现通过。
