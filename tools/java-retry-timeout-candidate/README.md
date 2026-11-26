# Java FILE/RAFT 重试超时保留候选

延续[现有恢复计划](../../docs/phase-0/java-retry-timeout-retention-candidate-plan.md)，不改变正式Rust/WP00依赖。

独立补丁修改三个固定Java类：FILE保留17；FILE/RAFT明确保留16/17供人工重试；提交不恢复已释放锁，回滚遵守原解锁策略。DefaultCoordinator两个既有回滚超时入口与恢复代码共用启动配置快照，避免配置provider变化导致同进程行为分叉。

`prepare_candidate.py`只生成或核对本候选补丁和独立manifest，不覆盖不同内容；JSON的CRLF/LF差异保持现有原字节。原Java源码/JAR、旧前导分片补丁/manifest及旧证据不改动。

`RetryTimeoutRetentionCandidateTest`包含37项；独立JVM PolicyProbe先冻结原协调器启动配置，再改变provider返回值，验证恢复仍遵守快照。35项受影响原测试单独回归。`native_observer.py`只注册TM并查询状态/锁，不创建事务、不注册RM资源、不争抢原资源通道。

`file_runtime.py`需要已验证的本次独立D盘构建，使用原JAR＋独立覆盖类、原持久化写入夹具及真实环境引导；验证Begin/16/17四种解锁策略，共六情形、12个完整TC进程。所有数据/构建留在D/E盘，只使用本次目录及loopback28091，结束停止自有TC。设置 `PYTHONDONTWRITEBYTECODE=1` 后运行Python测试或运行器。

[归档证据](../../docs/phase-0/evidence/local-java-retry-timeout-candidate/README.md)。这是候选及seeded真实FILE恢复证据；没有运行实际RAFT选举或FILE上的真实RM补偿，不能直接关闭完整恢复、原参考缺陷或生产Rust交付。
