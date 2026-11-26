# SDK 在途结果候选

对应[实施计划](../../docs/phase-0/go-java-v1-inflight-plan.md)及已接受 ADR-0008。0006 在独立 Go 副本实现显式 Java V1 **同步**请求的本地失败/未知结果分类、实际会话归属、响应入站快照和身份式清理。默认路径保留，原版及前五候选不修改。

使用已有工具/依赖缓存，在仓库根目录执行：

```powershell
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/go-java-v1-inflight/run.py
```

工具先冻结继承与本轮输入，再导入支持代码；从固定 Go archive 导出新 D 盘副本，应用0001–0005，运行19项行为对照，然后应用0006。完整回归为254项（235继承+19新增）、Rust117/静态检查及Python5项输入/代理审计。Java617项使用已认证父运行并校验冻结字节，**本轮不重新执行Java617**。

真实 SDK 经原 InitGetty、SessionManager、dubbo-getty loop、listener、response processor 和 future 工作。原 Java JAR 以独立前导分片 class overlay 启动，实际加载来源必须一致；SDK↔TC 都强制分片。Begin/Commit/Rollback 响应丢失后断连，以及保持连接且自然等待20秒的 Begin 超时，均须得到可识别的 UnknownOutcome；只查询 TC 状态，不重发事务。异步提交8可过渡至Finished15，查询次数按实际历史核对；不得伪造固定步数。

未知 Begin 的 XID 来自本测试代理看到的 TC 原始成功响应，供清理与验证使用。这不代表 SDK 已获得通用 Begin 幂等恢复能力。认证关闭、无真实数据库分支，仍不证明 RM/Undo、完整客户端、异步业务 API、TLS 或生产 Rust/WP-00 已交付。

常规测试涵盖另一会话的合法 RM 请求 ID 碰撞、伪装响应、已有 future 冲突、响应/关闭竞态、暂停处理器后的 ID 复用和旧 nil context。另有仅测试用的调度屏障材料 `guard_interleaving_probe.go.txt`；该材料须在独立 D 构建插桩、记录原/插桩/恢复哈希后删除，不能进入0006或冒充未插桩生产回归。

[证据入口](../../docs/phase-0/evidence/local-go-java-inflight/README.md)。结果只关闭已验证的候选子项，完整计划和现行冻结门禁保持有效。
