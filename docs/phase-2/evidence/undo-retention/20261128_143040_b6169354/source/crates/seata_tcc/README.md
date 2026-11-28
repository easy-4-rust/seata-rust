# seata-tcc

Phase 4 的独立 TCC 核心，事实源为 [TCC 契约](../../docs/phase-2/tcc-contract.md)。

当前提供原 `BusinessActionContext` 信封读取和输出、原 JSON 参数值字节保留、Java 空白/空字段/布尔标志语义，以及完整 Try/Confirm/Cancel Fence 决策。JSON 有 1 MiB 原字节和 64 层容器预算；诊断不输出原身份或参数值。原 Java 21 项实际向量、BMP 空白差分和 Rust 状态矩阵分别验证相应行为。

`FenceHandler::plan` 只生成持锁事务内的动作。它不执行 SQL、不声明事务已提交，也不证明原 TC 授权。数据库层必须将唯一插入或比较更新与业务放在同一事务，确认持久提交后才能发布成功。空 Cancel 的缺行观察不能绕过 SUSPENDED 唯一插入仲裁，重复 Try 不能重放业务。

正式 ResourceClient/RM 与 MySQL 持久业务/Fence 的显式实现位于 seata-db-mysql，选定原 Java TC 和故障恢复证据由项目 TCC 契约与 ledger 管理。本核心提供数据和异步能力契约，不将这些接口本身当作完整 TCC、生产支持或总计划完成证明。

动态业务可按需覆盖 `try_action_with_context`，默认实现继续委托既有只读 Try。`TccTransaction::report_context` 是可选的显式异步能力，默认明确返回不可用；具体资源必须负责报告版本、原回执及事务封闭。仅设置 delayReport 或 updated 不会发送网络请求，已确认报告也不证明本地 Try 已提交。
# util 参数共享

`BusinessActionContextUtil::add_context` / `add_context_map` 显式借用当前可变上下文与受控事务。忽略 null，同值不报告；只在 `delayReport == Some(true)` 时延迟，其他情况下参数变化会自动等待报告。成功才清除 updated；错误/取消由事务封闭原作用域，不盲重试。单独 `report_context` 保留 Java 的 updated 门槛。显式借用代替 ThreadLocal，避免异步任务迁移时串用上下文。

`ActionContextUtil::handle_action_context` 将对象/数组保存为 JSON 字符串，标量保留原值。RawValue 边界不携带 Java 集合类元数据，复合字符串内保存提供的原 JSON 字节（含精确数值与空白）；不是 Java 任意对象/反射注解或类型自动反序列化支持。批量预算拒绝保持原上下文不变。实际网络报告只由显式启用的适配器提供，核心 utility 不签发 TC/事务成功证明。
