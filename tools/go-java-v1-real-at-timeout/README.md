# 真实 AT 自然重试超时与持久重放

继续既有 [WP00 恢复计划](../../docs/phase-0/go-java-v1-real-at-timeout-plan.md)，不改变正式 Rust 的启动门禁。

`run.py` 使用已认证的 Go0001–0006副本、原 Java TC JAR及独立前导分片覆盖类、固定 JDBC8.0.33和已缓存 MySQL 镜像。它只操作新建且带归属标签的D盘数据及loopback28091–28093，不复用其他数据库。

一个真实AT分支经历：自然17 → TC/SDK/MySQL重启 → 人工重试真实补偿但丢失成功响应 → 再次重启 → 同分支安全重放。最终业务余额1000，TC global/branch/lock均空，Go RM保留原分支status1防悬挂Undo。所有SQL、原帧、依赖、实际加载类及进程身份逐阶段核对。

运行前设置 `PYTHONDONTWRITEBYTECODE=1`，使用Python执行此目录的 `run.py`。默认输出到仓库 `docs/phase-0/evidence/local-go-java-real-at-timeout/`，运行数据留在D盘，退出后停止本次容器与进程。运行器不安装工具或修改原始Java/Go源码。

`test_verify.py`验证规范化状态，`test_evidence.py`破坏真实证据副本验证原帧、SQL、控制台和操作顺序；正式运行会设置 `SEATA_AT_TIMEOUT_EVIDENCE` 指向本轮证据。未设置时，测试仅使用旧D盘trial构造单位fixture，明确不作为正式认证。

CommitRetryTimeout16、FILE/RAFT安全恢复、全SQL原子性/故障矩阵、混合语言资源标识及生产Rust均未由此完成。审查和实测证据见[归档目录](../../docs/phase-0/evidence/local-go-java-real-at-timeout/README.md)。
