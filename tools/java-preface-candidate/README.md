# Java V1 前导分片独立候选

对应 [JAVA-V1-PREFACE-001](../../docs/phase-0/java-v1-preface-fragmentation.md) 和[实施计划](../../docs/phase-0/java-preface-candidate-plan.md)。仅在全新 D 盘源码副本应用可审查补丁，不修改参考仓库、原 TC JAR 或 Go SDK 候选。

在仓库根目录运行（使用已有 JDK 17、Maven 3.9.9、Go、Rust 和本地依赖缓存，不安装工具）：

```powershell
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/java-preface-candidate/run.py
```

流程为原版已有回归 → 原版 48 项目标测试的真实失败 → 应用补丁 → 完整候选回归。RED 因核心测试失败而提前结束，不能要求尚未执行的下游报告；原版下游回归必须先独立通过。GREEN 的测试身份必须等于 baseline 与 RED 的并集。

原生产源/POM/resources 全部逐项审计，只允许清单中的五处修改与一个新接口。Git 原始 CRLF 在独立副本的五个补丁目标上先经哈希校验再转 LF。既有公共 `ProtocolDetector` 不修改；Lombok 1.18.36 仅作为 provided/optional 编译依赖。

执行类从新鲜构建复制，实际 JVM 日志必须显示五个候选主类来自该 overlay。原 JAR 先复现首字节拒绝，再用 overlay 验证逐字节 V1、连续帧、未知前缀、HTTP/1 404、HTTP/2 SETTINGS，以及实际 SDK 双向强制分片、17 请求/响应、重复/晚到/未知结果、重连与原 XID 回滚。HTTP 结果只认证协议选择。

既有 Java 协议测试、475 项 Java 基线、117 项 Rust 独立探针、235 项 Go 及 Python 来源/审计检查均需实际执行。Rust 固定夹具全部冻结，五个测试目录显式指定，避免外部环境改变输入。日志、报告、输入与执行字节保存至 [证据目录](../../docs/phase-0/evidence/local-java-preface-candidate/README.md)。

该候选只认证 V1 的新分片行为。V0 下游不足完整头时的继承问题、V2 专门分片认证、原始制品发布、完整客户端、真实 RM/Undo、生产 Rust、WP-00 及 Phase 1–7 仍未完成。
