# TIMESTAMP：强杀原进程后的独立 RM 恢复

落实[既有恢复契约](../../mysql-timestamp-restart-contract.md)，见 [ledger](../../wp05-mysql-timestamp-restart.json)。选定显式 UTC 配置的进程恢复已补验；DATE/TIME、其他配置/格式、完整 WP05/WP07 及全计划继续实施。

TM 监督进程只负责原全局事务与进程编排。业务 RM 子进程完成六条 DML、原事务 Undo/SHA/见证与报告后，监督进程通过实际句柄强杀并等待退出。新的 RM 子进程读取持久尝试记录、见证、Undo、原日志及匹配配置，重新登记资源后接收原 Java TC 对同一分支的回调。恢复者没有继承业务进程的镜像对象。

| 验证项 | 实际证据 |
|---|---|
| Rust / 配置 / 方向 | 1.99、1.85 × UTC 编号 3/4 × Commit/Rollback，八笔全局事务 |
| 实际进程 | 每例监督、业务和恢复进程，共 24 个 Rust 进程实例；业务进程实际非成功退出，新 RM 和监督进程成功退出 |
| 外部 OS 核验 | Windows CIM 的 PID/父 PID、程序路径、命令行、创建时间，以及保留句柄的退出状态 |
| 旧进程死亡 | 在观察到新 RM 时，旧进程句柄已为终态；原进程不是仅关闭对象或伪造状态 |
| 原分支 | XID、BranchId、资源、配置、分支登记和报告均保持原身份；没有第二个业务分支 |
| 回调 | 两次资源连接登记成功，业务分支在旧连接登记，全部二阶段回调使用新连接 |
| 值与持久状态 | 原 Java 对象图、0–6 精度、微秒、SQL NULL、上下界、TIMESTAMP 主键、业务值、Undo/分支终态与日志 CRC/Completed |
| 最终工作区 | 每个 Rust 812 通过、403 忽略、187 个成功汇总；受影响 crate 的全部目标/特性严格 Clippy 通过 |
| 证据削弱 | 160 项原格式/TC/物理状态检查和 104 项进程证明削弱，共 264 项被拒绝 |
| 复位授权 | 两个有效 socket 事件获准，11 个错误授权/异常被拒绝；实际复位须绑定旧连接和原授权字节 SHA |

403 个忽略项不是 403 个通过；本次实际执行的选定原生测试另外由运行器核对。生产 Rust、Cargo 配置和依赖沿用 `d30dae995eaf699eeb2ad8ba5dca0f1a5dbf41a9`；新增的是故障验收夹具和工具。原 Java TC、JAR 与 18 个重新编译的原 JDBC/Undo 源码保持既有固定输入，未替换协调逻辑。TC 没有在此矩阵中被强杀，不据此宣称 TC 与 RM 联合宕机通过。

权威现场：[java_20261128_101908_c8c94331](java_20261128_101908_c8c94331/result.json)，catalog SHA-256：`c452c3574f022a62d68ce4952a130b5e29456c65c348a727eb90affaa85c740b`。

最终核验：[validation_20261128_final](validation_20261128_final/result.json)，catalog SHA-256：`33bb5f789d346cc6dbcc4bb1818fa33fb9fda20339133d06284c74e2580b01fc`。它绑定原始矩阵、双 Rust 回归、复位授权反例、最终工具源码及数据保留核验。复验命令：

```powershell
python -B tools/formal-source-junit/validate_timestamp_restart.py `
  docs/phase-2/evidence/mysql-timestamp-restart/java_20261128_101908_c8c94331 `
  c452c3574f022a62d68ce4952a130b5e29456c65c348a727eb90affaa85c740b --weakenings
```

普通观察器会把强杀的 10054 复位当作错误；本补验使用单独适配器，仅接受实际业务进程被观察存活后签发的十秒授权。独立核验继续要求摘要成功、无普通错误，并把预期复位限制到旧业务连接。观察器复位错误没有被静默忽略。

诊断记录保留：`100218_f8a9e81b` 首笔及 `100535_9b22b7fc` 矩阵的业务恢复完成，但普通观察器报强杀复位，因此不计入验收；`101006_d872f5ae` 是重复脚本参数导致的观察器启动失败；`101158_ad4ef205` 是通过独立核验的 Clippy 收尾前矩阵；`101643_efd80532` 是夹具混用 String/Xid 转换产生的编译失败，不是 TDD 行为反例。早期 `101030` 回归因夹具 Clippy 失败、`101653` 回归因该编译问题失败；最终只采用 `101819_0d7fb5d8` / `101819_8b43d61d` 和上述最终原生矩阵，不把诊断运行累计为通过数量。

运行期间只对明确停止且容器已移除的旧夹具数据做 NTFS 无损压缩，逐文件 SHA 前后及最终核验一致；没有删除数据或缓存。见 [E 盘数据保留](retained-data-compression_20261128.json)和 [D 盘诊断数据保留](diagnostic-data-compression_20261128.json)。所有本轮专属容器和观察到的子进程均已终止。

下一步推进 DATE/TIME 的完整值域及 Java Undo 兼容，不用当前 TIMESTAMP 结果推断它们完成；完整 SQL、写入器准入、生产部署和后续 Phase 5–7 保持原计划范围。
