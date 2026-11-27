# TCC 上下文与 Fence 决策核心

事实源：[TCC 契约](../../../phase-2/tcc-contract.md)。最终目录与可信摘要见 [machine ledger](../../tcc-core.json)。本页只验收独立核心，不能代替持久 Fence 或正式 TCC 资源支持。

原 Java `BusinessActionContext`、`BusinessActionContextUtil`、`ActionContextUtil` 和 Fence 常量直接取固定 Git 原字节编译，类加载日志核对上下文来自刚编译的原类。其余依赖使用固定服务器产物及已记录 SHA 的原客户端依赖；独立进程不加载需要 Spring 容器的服务器 Boot 配置提供者，使用封存的 file 配置。没有改写原对象。实际执行 21 个上下文输入、4 个 Fence 码及 JDK 对 BMP 的空白分类观察。

Rust 1.99 / 1.85 每版 6 个组件测试覆盖原 21 输入、全部 63,488 个 BMP 有效标量、1 MiB 字节边界、64 层结构预算、原 long 极值、空字段、布尔标志、重复键最后值、原参数精度和 15 个 Fence 操作/状态格。业务 JSON 值保留原字节，大整数和小数不会经 f64 转换；Debug 与错误不输出原 XID、资源名或参数值。

Fence 操作条件对照固定 `SpringFenceHandler` 源码：唯一插入 TRIED 后 Try，TRIED 上 Confirm/Cancel 的比较更新，同方向完成复用，空 Cancel 必须插入 SUSPENDED，任何已有状态拒绝重复 Try。这里测试的是计划生成；数据库原子插入/更新、业务调用及提交尚未实现，源码核对也不冒充原 Java 数据库 Fence 执行。

七种生产源码错误变体在独占 D 盘工作区和独立 Cargo 产物目录实际编译、运行：放宽字节/深度预算、参数转浮点值、把字符串当 Fence 标志、允许迟到 Try、把相反终态当成功、错误 Unicode 空白。双版本合计 14 个指定行为测试失败；编译失败不计入行为反例。正式工作区源码不被变体改写，最终封存核对字节一致。

双版本完整工作区各 791 passed / 372 ignored，175 项成功 harness 汇总；TCC/XA/MySQL 严格 Clippy 通过。独立校验器核对原 Java 源、原始观察和生成夹具、实际反例源码与失败输出、完整工作区汇总及当前源码封存。

诊断保留：初始核心的四项失败证明目标行为缺失；Java 联试曾发现服务器依赖集不包含客户端 RM/Tx API，以及独立 JVM 缺少 Boot 环境/file 配置；运行器已使用原依赖和显式隔离配置。首轮字节预算反例触发测试常量溢出，未被计为行为反例；改用独立固定边界输入。第二次基线暴露共享 Cargo 目标目录的变体产物复用，已改为独占目标，正式 TCC 产物清理并重建。最终仅使用 ledger 指定的 Java、反例和两版回归目录；其他目录均为诊断或补强前运行。

后续继续正式 TCC RM/原回执交接、MySQL 业务与 Fence 同事务、空回滚和迟到 Try 实际竞态、未知提交/进程/数据库恢复、原 Java TC 整笔事务、动态上下文报告与生产验收。完整 TCC、XA/AT 剩余门禁及 Phase 5–7 继续开放。
