# 真实 AT＋TCC 提交重试超时

按[既有WP00增量计划](../../docs/phase-0/go-java-v1-real-commit-timeout-plan.md)验证REC-TIMEOUT-008的选定混合事务链路。纯AT异步提交不作为自然16证据。

完整Go SDK注册原AT/TCC RM：AT真实修改业务行并保存Undo；TCC业务在原XID/BranchId下预留10个单位。代理阻断原TC Commit指令，让固定Java TC自然进入16。TC/SDK/MySQL重启后，人工重试使真实AT清理Undo、真实TCC确认业务；仅丢弃TCC成功响应。再次重启和人工重试原分支，TCC确认仍10、效果计数仍1，TC记录/分支/锁清空。

TCC持久幂等由本夹具业务的本地事务、唯一键和行锁实现，不能归功于原SDK为任意业务提供自动幂等。夹具不替换原RM或合成协议成功响应。

设置 `PYTHONDONTWRITEBYTECODE=1` 后运行本目录 `run.py`。构建/数据位于D盘，证据默认位于仓库 `docs/phase-0/evidence/local-go-java-real-commit-timeout/`。仅创建新的带归属标签MySQL容器、私有数据库及loopback28091–28093；退出后停止本次资源。脚本不安装工具或修改父证据、原Java/Go源码。

27项Python测试含状态/持久结果及真实原始证据破坏测试。正式运行将 `SEATA_COMMIT_TIMEOUT_EVIDENCE` 指向当轮输出；未设置时仅从旧D盘trial复制单位fixture，不构成正式认证。原Go/Rust/Java单元测试范围沿用已认证父证据，另行标明本轮未重跑。

[正式证据](../../docs/phase-0/evidence/local-go-java-real-commit-timeout/README.md)。FILE/RAFT安全恢复、已知重试指标异常、混合语言资源标识、全SQL/崩溃矩阵、完整TCC Fence及生产Rust/WP00仍开放。
