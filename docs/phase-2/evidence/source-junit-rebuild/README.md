# 原 Java JUnit 基线重新构建（2026-11-27）

规格事实源：../../source-junit-contract.md。固定 Java Git cba73e180fd8896a99df6415b859d275122feaa7。旧三个 evidence junction 的目标仍缺失；本目录保存新执行，不能替代未恢复的旧历史现场。

- run_20261127_021515_800aeb39：4,269 个原 Git blob 原字节导出，原 Maven reactor 24 模块 test-compile 成功。literal-source.zip 的 SHA-256 与 build-command.json 相同；编译前后原文件全部未变。
- runtime_20261127_022128_dd3614d0：原 Jupiter 5.8.2 / JDK 17.0.12 实际执行。SQL parser core 76 例、RM datasource 1,212 例通过；另 39 例由原 JDK17 注解禁用。共 1,327 个具体案例，运行时引用 1,135 个声明方法。
- closure-audit.json / rebuilt-Java-cases.jsonl：原 final plan、事件生命周期、禁用分区及真实 JVM 进程闭合检查。34 项证据削弱全部拒绝。RM 三个原压力子 JVM 均保留默认 rounds、每子进程 200 线程；额外 ByteBuddy attacher 单独观察。父 JVM 使用原 HarnessGuard；子 JVM 环境探针不等于子进程具有同一 SecurityManager 边界。

重放命令（仓库根目录；已安装的 JDK/Maven 和固定 Git oracle 为前提）：

    python -B tools/formal-source-junit/rebuild_baseline.py
    python -B tools/formal-source-junit/run_rebuilt.py --build <新 build 目录>
    python -B tools/formal-source-junit/audit_rebuilt.py --runtime <新 runtime 目录>
    python -B tools/formal-source-junit/authenticate_rebuilt.py

新入口只改变证据输出根目录，原入口默认行为保留。Maven 构建/依赖写入 D，Git 证据写入 E，C Maven 缓存只读。

本批仅重建 Java 执行证据，不重置既有 Rust 匹配账本。未完成：独立 AST 声明清单重建、全部 runtime 输入可搬移封存与离线重放、历史现场恢复、独立人工/另一 Agent 审查、39 个禁用案例的其他 JDK profile、其余原案例 Rust 逐案对照。WP05、全部 V1 与总计划仍未完成。Rust 生产/测试源码未修改，上一提交双版本 717 passed / 318 ignored 结果沿用，未冒充本批重新执行。

依赖核验：dependency_auth_20261127_022357_e4773a97 的 1,221 项 Maven POM/JAR 均与 exact-version Maven Central SHA-1 匹配，同时记录本地 SHA-256。原官方校验响应按原文件名封存于 published-checksum-responses.zip；依赖 JAR 实体仍位于 D Maven repository，本批不宣称已封存完整离线运行包。
