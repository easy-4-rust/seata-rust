# Java 独立兼容候选

基于固定提交 `cba73e180fd8896a99df6415b859d275122feaa7`，补丁仅用于独立源码副本。参考仓库和原 TC JAR 保持不变，默认 Java 制品仍有首字节拒绝问题。

[0001](cba73e180fd8896a99df6415b859d275122feaa7/0001-incremental-protocol-preface.patch) 新增可选渐进探测接口，保留旧 boolean 接口；隔离探测器索引、按可读区判断前缀，为 V1 版本选择至少等待三字节，并覆盖 HTTP/1 与 HTTP/2 前缀。未知协议日志仅记录长度。Lombok 为固定版本的编译期依赖。

[文件/依赖哈希](cba73e180fd8896a99df6415b859d275122feaa7/manifest.json)、[复现工具](../../tools/java-preface-candidate/README.md)、[验收计划](../../docs/phase-0/java-preface-candidate-plan.md)、[实际证据](../../docs/phase-0/evidence/local-java-preface-candidate/README.md)。应用前必须按工具说明在独立副本校验并转换目标 CRLF；不能直接在原参考仓库操作。

这是候选验证，未合入或发布原 Java 工程，也不替代完整客户端、V0/V2 专项认证或生产 Rust 交付。


## 0005：显式复制分支上下文更新

[补丁](cba73e180fd8896a99df6415b859d275122feaa7/0005-replicate-explicit-branch-context-updates.patch)及独立manifest覆盖BranchTransactionDTO、RaftSessionManager、UpdateBranchSessionExecute三个真实对象。详见[计划](../../docs/phase-0/java-raft-context-replication-candidate-plan.md)和[真实证据](../../docs/phase-0/evidence/local-java-raft-context-candidate/README.md)。原JAR/固定仓库未改；本运行使用前导/保留/reload20生产类＋本补丁3类，共23生产类，未加入0004指标类，不能据此认证RAFT指标。

显式TRUE复制当前context（支持值/空/null）；历史missing/null/false标志保持旧语义。同测试class Java25原类11失败/14控制、候选全过，原类真实生成705B旧消息兼容；真实更改Prepare context后三节点完整恢复通过。仅统一补丁组，禁止据此宣称混合版本滚动升级或历史丢失上下文恢复；原参考问题继续OPEN。

## Java AT 写入器准入

固定客户端可在业务 SQL 前校验物理 MySQL 身份、原 Seata 资源 ID 与 Connector/J 映射，并持有原事务的绑定锁。[接入说明](writer-admission/README.md)。
