# 全局状态协议与辅助分类验证

本工具只扩展 Phase 0 参考证据，沿用[本项计划](../../docs/phase-0/java-global-status-contract-plan.md)和既有 Java 权威基线；不实现生产 Rust、Go TM 状态机或新兼容补丁。

```powershell
$env:PYTHONDONTWRITEBYTECODE = '1'
$env:PYTHONUTF8 = '1'
python tools/java-global-status-contract/run.py
```

依赖既有固定 Java/Go 源码、JDK17、本地缓存和 0001–0006 已验证副本；不安装、不初始化、不更换分支、不启动网络服务。新编译产物写入 D:/codex-builds，原始证据写入 E: 仓库。

实际执行原 GlobalStatus 和 SessionStatusValidator，检查 21 状态的九种辅助分类及 441 个状态对；Java 原 serializer 生成四种响应 × 21 状态 × 成功/失败，共 168 项。Go 原版与显式入口接收相同实际 JVM 输出，同时独立构建对象进行编码；Python 另按已确认的 V1 字段格式核对字节。Java 枚举拒绝 21–255，四种响应的 940 个未知状态变体用于观察 Go 默认与显式入口的差异。

运行前递归冻结实际动态 Python 模块及 Java/Go 输入；原 Go 714 文件、完整候选来源、Java 2122 生产/资源/POM 及实际 classpath 依赖前后比对。16 个关键实际 Java class 同时匹配认证原 JAR 和固定参考 target；归档 Probe.class、两个 Go binary、编译参数与工具版本，保存启动前哈希。11 项语义证据测试先确认十项缺失门禁失败；来源集合另五项先确认四项门禁失败；实际副本六项先确认来源绑定/两份Probe同改两个漏检，其余四控制通过；最终合计 22 项通过。最后一组为在验收前运行改读 execution.json，明确保留前后测试源与日志，不声称测试源完全相同。

`seal.py` 只汇总已结束的运行，创建文件目录清单；RED 明确是禁用校验器门禁的对照，不是原 Java/Go 生产功能失败。最终结果详见[运行证据](../../docs/phase-0/evidence/local-global-status-contract/README.md)。状态数字往返、辅助分类、校验方法允许都不等同于业务执行、调度或转换可达；GO-V1-STATE-001 继续 OPEN。
