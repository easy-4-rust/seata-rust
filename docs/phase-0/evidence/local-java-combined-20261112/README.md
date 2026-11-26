# Java 联合测试验收记录 — 2026-11-12

本轮补齐 `phase09-local-validation-plan.md` 中此前未完成的本地联合测试验收，沿用 `java-tc-live-integration-plan.md` 和原协议黄金向量。没有创建新规格体系。

## 实际结果

固定 Java SHA：`cba73e180fd8896a99df6415b859d275122feaa7`。JDK 17.0.12，Maven 3.9.16，使用已有临时 Maven Central 配置及 E 盘依赖缓存。

| test-suite/test-new-version 测试类 | Tests | Failures | Errors | Skipped |
|---|---:|---:|---:|---:|
| ProtocolV1GoldenFrameTest | 6 | 0 | 0 | 0 |
| AbstractTCInboundHandlerExceptionTest | 8 | 0 | 0 | 0 |
| SessionHolderRetryTimeoutRecoveryTest | 3 | 0 | 0 | 0 |
| 合计 | 17 | 0 | 0 | 0 |

Maven 91 个模块 `BUILD SUCCESS`，退出码 0，耗时 5 分 19 秒。上游依赖模块中的独立 serializer 测试另执行 6 项并全部通过，不能把这 6 项重复计入联合模块的 17 项。这里执行的是上述指定测试集，并非上游所有测试。

两个注册帧在本地联合模块中都通过完整字节相等断言；此前远程 CI 的注册帧长度差异没有复现。本轮没有修改 Fixture、断言或任何上游受版本控制的源码，不声称找到远程失败根因或修复远程 CI。`upstream-tracked-diff-exit.txt` 为 0，diff 文件为空；上游构建默认激活的 Spotless 没有产生 tracked diff。

## 文件与可复核性

- `maven-baseline.log`、`console-baseline.log`、`exit-baseline.txt`：实际成功执行的完整记录。
- `TEST-*.xml`、对应 `.txt`、`test-summary.json`：从联合模块复制的三份原始 Surefire 报告和汇总。
- `execution.json`：版本、选择器、配置来源和范围。
- `overlay-before.json`：运行期间记录的测试文件 SHA-256；文件名中的 before 不表示早于 Maven 启动。
- `tested-*.java`：实际执行的覆盖测试源码副本。
- `manifest.json`：证据文件 SHA-256。
- `maven-combined.log`、`console.log`、`exit.txt`：首次命令因 PowerShell 拆分未加引号的点号属性失败，尚未运行测试。成功命令已为该参数加引号，不能把首次命令解析失败解释为 Seata 测试失败。

Oracle 中原有的 ProtocolV1GoldenFrameTest 与 Rust 工具目录当前版本仅差 ByteBufUtil 和失败时的十六进制诊断信息；本轮保留 Oracle 原文件，未覆盖它。Golden 字节、断言与六个测试案例相同。新增另外两个测试覆盖文件；原有 serializer 模块测试覆盖文件保持不变。

## 复跑命令（PowerShell，已有 Oracle 及覆盖测试）

```powershell
$env:JAVA_HOME = 'D:\Java\jdk-17.0.12'
$env:PATH = "$env:JAVA_HOME\bin;$env:PATH"
$env:TEMP = 'D:\Temp'
$env:TMP = 'D:\Temp'
$env:MAVEN_OPTS = '-Djava.io.tmpdir=D:/Temp'
Set-Location 'E:\workspaces\workspace-github-easy-4-rust\seata-java-phase09-oracle'
& 'D:\Java\maven\apache-maven-3.9.16\bin\mvn.cmd' `
  -s 'E:\workspaces\workspace-github-easy-4-rust\seata-rust-phase09-validation\docs\phase-0\evidence\local-phase09-live\maven-central-settings.xml' `
  -B -ntp -pl test-suite/test-new-version -am `
  '-Dtest=ProtocolV1GoldenFrameTest,AbstractTCInboundHandlerExceptionTest,SessionHolderRetryTimeoutRecoveryTest' `
  '-Dsurefire.failIfNoSpecifiedTests=false' test
```

## 完成情况与下一步

- 已完成：本地联合指定测试集验收，以及证据归档。
- 未完成：远程 CI 原失败环境的根因定位与复验；FILE 重启恢复、认证、数据库 AT 补偿等验收。
- 兼容性：未改变运行时行为、黄金向量或公共接口。
- 规格状态：本地联合验收由“超时，未知”推进为“17/17 通过”；Phase 0 继续开放。
- 同名聊天正在实现 Rust BranchRegister/RM 二阶段回调，本轮不修改它的文件，不将它尚在进行的结果记为本轮完成。
- 没有创建分支、提交、推送或变更远程工作流；所有新增证据在 E 盘。
