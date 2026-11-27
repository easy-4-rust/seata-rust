# 动态报告的上下文核心依赖

规格事实源为 [TCC contract](../../../phase-2/tcc-contract.md) 的“动态上下文的核心依赖”。本增量实现 `BusinessActionContext` 的受控业务参数变更、Java 可选 updated/delayReport 状态和变更跟踪；不把本地标志当作已发送或已持久报告。

## 实现与实际验证

- 固定 Java 源码 pin `cba73e180fd8896a99df6415b859d275122feaa7`，原 `BusinessActionContext`、其内部跟踪映射及 util/ActionContextUtil 被新编译、实际加载执行。观察器得到 46 项：30 个值比较、15 个跟踪生命周期、1 个 util 延迟添加对象的后续基线。Rust 比较前 45 项；最后一项不计作 util 自动报告已实现。
- 新增 `get/set_updated`、`get/set_delay_report`、初始化后开启跟踪，以及受控 `put/add/remove/clear/set_action_context`。直接 put 支持 null，add 忽略 null；删除已有 null、空映射和同值替换按 Java 区分。替换映射保留跟踪开关而不自行标记变更。原身份仍不可写。
- 所有保存和序列化使用原 `RawValue`；失败、字节或深度预算超限的变更原子拒绝，保留旧值和原标志。新对象不返回可变映射引用，也不公开原连接或报告权限。
- 初始 RED 为缺少新 API 的编译拒绝，保留原日志。随后真正执行目标行为测试，并在独占 D 盘复制工作区中执行 17 类错误生产源码，每版 17 次实际行为失败，共 34 次；其中 10 类覆盖本增量，不能把编译失败算作这些行为反例。
- Rust 1.99、1.85 工作区各 795 passed / 384 ignored / 178 成功测试汇总，严格 TCC/XA/MySQL Clippy 通过。恢复矩阵、实际 MySQL、正常原 Java TC 与原 TC 回调故障另行回归；源码、命令和证据见 [本轮 ledger](../../tcc-context-mutation.json)。

## 相等性与原字节

原 Java JSON Provider 把小数和指数解析成 Double。为了保留其变更标志语义，比较时使用 Double 的位相等性，包括正负零、溢出和精度合并；Integer/Long/BigInteger 不相互强转。对象比较不依赖键顺序，数组比较保留顺序。

比较结果不能代表原字节相同。例如两个超出 Double 精度的小数、`1e10000` 与 `2e10000` 在固定 Java 中比较相等；Rust 的 updated 标志按该观察保持未变，但实际新值仍以完整原 JSON 字节替换和导出。后续持久报告必须独立核对原载荷和报告版本，不能只用 updated 来判断参数是否一致、报告是否成功或业务是否提交。

原 Provider 在观察器重新序列化映射时会输出 `@class` 元数据并忽略 null；差分测试只在该观察结果中排除序列化元数据，生产解析器不会擅自删除用户的同名 JSON 键。

## 尚未接通的报告链路

现有 Try 业务契约继续接收只读上下文。后续需要完成：可变 Try 上下文与立即/延迟报告交接；原登记身份、完整新载荷和持久报告版本的区分；单次 Registered 状态报告；明确拒绝、丢响应、取消和进程故障；旧 Pending 与合法新参数回调的稳定身份校验；原 Java TC 接收新参数并完成 Commit/Cancel 的原始协议和独立 SQL 验收。不得通过全局忽略 application_data 冲突或修改 Java Undo 格式来绕过这些要求。

本增量不是完整动态上下文报告，也不关闭完整 TCC 或总计划。生产权限、保留清理、负载、运维和其他 Phase 0–7 门禁继续保留。
