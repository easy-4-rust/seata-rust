# seata-tcc

Phase 4 的独立 TCC 核心，事实源为 [TCC 契约](../../docs/phase-2/tcc-contract.md)。

当前提供原 `BusinessActionContext` 信封读取和输出、原 JSON 参数值字节保留、Java 空白/空字段/布尔标志语义，以及完整 Try/Confirm/Cancel Fence 决策。JSON 有 1 MiB 原字节和 64 层容器预算；诊断不输出原身份或参数值。原 Java 21 项实际向量、BMP 空白差分和 Rust 状态矩阵分别验证相应行为。

`FenceHandler::plan` 只生成持锁事务内的动作。它不执行 SQL、不声明事务已提交，也不证明原 TC 授权。数据库层必须将唯一插入或比较更新与业务放在同一事务，确认持久提交后才能发布成功。空 Cancel 的缺行观察不能绕过 SUSPENDED 唯一插入仲裁，重复 Try 不能重放业务。

正式 ResourceClient/RM 接入、MySQL 持久业务/Fence、故障恢复、动态参数报告和原 Java TC 整笔事务仍在后续实施中。因此此组件存在不代表完整 TCC、生产支持或总计划完成。
