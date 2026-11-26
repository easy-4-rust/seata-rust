# 原 Java 测试基线：AST 对照与完整便携重放

规格事实源为 [source-junit-contract.md](source-junit-contract.md)。本轮重建所选 RM datasource / SQL parser core 的 Java 基线，剩余 Rust 逐案迁移及独立审查继续开放。旧三个 junction 指向的物理现场仍缺失。

## 当前结果

| 检查 | 实际结果 |
|---|---|
| 固定 Java Git | `cba73e180fd8896a99df6415b859d275122feaa7` |
| 原字节源码 | 4,269 文件，原 Maven reactor 24 模块测试编译成功 |
| 独立 AST 库存 | 189 份 Java 测试、1,135 个声明方法、七份原资源 |
| 声明与真实案例绑定 | 1,096 个方法执行、39 个方法按原 JDK17 注解禁用；共 1,327 个具体案例 |
| 便携重放 | 1,288 例通过、39 例原注解禁用，无失败/中止/容器失败/父 JVM Guard 拒绝 |
| 压力子 JVM | 三个原子进程，原默认 rounds、各 200 线程、原 30 秒用例超时 |
| 输入封存 | 5,623 项、89,884,584 字节；ZIP 64,384,990 字节 |
| Maven 依赖真实性 | 上轮重新解析的 1,221 项 POM/JAR 匹配 Central 校验响应；便携闭包内每个依赖 JAR 均核验，包括原 reactor 的 Druid JAR 别名 |
| 证据削弱 | 26 项事件 + 八项 JVM 进程 + 三项声明集合 = 37 项均拒绝 |
| 工具回归 | 六项通过；类加载历史丢失变异在目标检查失败，正式代码通过 |
| 清理 | H2 原相对目录已删除，六个实际 JVM 均已退出，全部封存输入执行后原字节不变 |

新目录只从封存包恢复源码、编译类、原资源、完整类路径 JAR 和观察器；执行时仅调用固定 JDK 17.0.12，不调用 Maven、旧构建目录或原 Java oracle。类加载日志逐条核对 Seata 类来源，运行时 ReflectASM/Fory 生成类须绑定已加载的原类和已认证生成器 JAR。Jar 内原 LICENSE/NOTICE 保留在归档中。

父 JVM 使用既有 HarnessGuard。子 JVM 探针记录真实进程/父进程、类路径、JDK、home/temp/hosts 及压力属性；这些观察不等于子 JVM 具有相同 SecurityManager 边界。本轮未增加生产沙箱或 TLS 能力。

## 证据与可信摘要

均位于 [source-junit-rebuild](evidence/source-junit-rebuild/)：

- `inventory_20261127_023641_d3a38c08`：独立原字节解压、JDK AST、七份资源及原运行案例绑定。
- `portable_inputs_20261127`：完整 `runtime-inputs.zip`、逐成员 SHA-256、原路径映射、有序类路径及配置。`manifest.json` SHA-256 为 `1ac8fcff14d29fd25bf42d61b4db07b421d1a84cc97c87e35fa7f0a237ebefd7`，验证工具固定该权威摘要，不能用自我重算的清单替换。
- `portable_replay_20261127_024305_7c82945c`：最终完整现场、事件、JVM、加载来源、执行工具快照、输入复核和退出证据。
- `qualification_20261127_024443_e947faab`：交叉核对后的 `source-method-bindings.json`、`concrete-source-cases.json` 和 37 项反例。目录 `catalog.json` SHA-256 为 `9251d923bc6070dfd202ba417376948aca5db87dceb3dc5e6d88e1730db498f6`。
- `tool_validation_20261127_024516_68b68455`：隔离变异失败及正式工具六项回归原输出。

新证据各有自己的目录清单。上轮 `evidence-files.json` 只封存当时的构建/首次执行/依赖核验快照，保持原字节，不能当作后来所有增量文件的全集。机器入口为 [wp05-source-junit.json](wp05-source-junit.json) 的 `current_rebuilt_Java_baseline`；旧顶层历史计数及路径继续明确标为历史。

首次 `portable_replay_20261127_023955_84045e73` 的 JUnit 通过，但后续类来源审计失败，作为诊断保留，不计完整现场。原因是同名类后续由测试动态定义，字典覆盖了早期原 reactor 加载记录。修正为保留并验证完整加载历史，未放宽生成器或原类来源规则。第二次 `portable_replay_20261127_024127_4ba1019b` 验证修复；最终目录另封存执行工具并增加真实 PID 退出审计，不累加成更多不同原案例。

## 重放

在 Rust 仓库根目录使用已安装的固定 JDK；输出只写 D/E：

```powershell
python -B tools/formal-source-junit/inventory_rebuilt.py
python -B tools/formal-source-junit/replay_rebuilt.py
python -B tools/formal-source-junit/check_rebuilt_tools.py
```

便携重放不需要 C Maven 缓存、D 原构建目录或原 Java oracle。另行重新编译原 Maven reactor 的入口仍为 `rebuild_baseline.py`，其网络依赖解析与从封存编译产物重放属于不同验证。

当前固定证据的程序交叉核验：

```powershell
python -B tools/formal-source-junit/qualify_rebuilt.py `
  --inventory docs/phase-2/evidence/source-junit-rebuild/inventory_20261127_023641_d3a38c08 `
  --inventory-catalog-sha256 b5da7de075ee1ec6f20360d32cb06eb27b279a7222d73893619e526576bb705b `
  --runtime docs/phase-2/evidence/source-junit-rebuild/portable_replay_20261127_024305_7c82945c `
  --runtime-catalog-sha256 189d436f25005e1241c6706ebe95c51c0c0119ce21701c8b04c649b615fb8d17
```

## 尚未完成

当前检查为程序核验，独立人工或另一 Agent 审查门禁仍开放。原 Surefire `test` 生命周期、其他 JDK profile 与全部被禁用方法体尚未执行，整个 Java 项目的其他模块测试不在此基线内。

Java 执行成功不能证明 Rust 逐案匹配。本批不改 Rust 源码，不重跑或重算全局 Rust 测试与 MATCH 总数，不重置此前已验证的匹配记录。WP05 十四项、WP06、WP07、完整 V1 和后续 Phase 仍须分别验收。
