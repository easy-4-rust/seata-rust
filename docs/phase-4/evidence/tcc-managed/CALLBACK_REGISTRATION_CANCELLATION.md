# TCC 原回调故障、登记未决与主动取消

规格事实源为 [TCC contract](../../../phase-2/tcc-contract.md)。本轮只扩充测试、运行器、独立校验和证据；不修改生产 API、迁移、Java Undo 格式或 AT/XA 历史记录。

## 实际验收范围

- Rust 1.99 与 1.85 各执行八项真实 MySQL TCC 用例。新用例覆盖登记前六类输入拒绝、明确登记拒绝、请求已发送后丢失、已到达登记请求的主动取消与迟到成功回执，以及原 Try 在提交前和真实 COMMIT OK 被扣留后的主动取消。
- 未取得成功回执的调用方 `branch_id` 保持 null。迟到原 TC 回调的编号独立记为 `callback_branch_id`；不能把测试对端预选编号作为客户端已取得的成功回执。
- 提交前取消通过实际 InnoDB 锁等待证明旧事务退出后才建立 SUSPENDED；提交响应丢失后取消通过原 COMMIT/OK 字节和独立 TRIED/业务查询证明已提交，再由原 Cancel 补偿。两者最终业务为空、迟到 Try 被拒绝。
- 固定原 Java TC、原数据库 Session Store 上执行八项故障事务：两种 Rust、Confirm/Cancel、复制原回调或丢弃真实分支终态响应后断连。客户端仅发一次 End；原 TC 自行重试，最终由原 TC 成功日志、原始报文、Fence、业务和 general_log 联合验证无业务重放。另有四项正常原 TC 全局事务回归。
- 两种 Rust 的工作区各 791 passed / 382 ignored / 177 成功测试汇总，严格 TCC/XA/MySQL Clippy 通过。ignored 为需显式环境的测试；其中本轮选定真实用例由独立运行器实际执行。
- 独立校验主动削弱证据：MySQL/正常 Java 154 项，原 TC 故障 96 项，共 250 项，均必须被拒绝。

## 封存与重现

最终输入、源码快照、逐命令输出、原始包、独立数据库事实和清理结果见 [本轮 ledger](../../mysql-tcc-callback-registration-cancellation.json) 的 authority。执行校验命令与校验器副本封存在 `validation_20261128_callback_registration_final`。早先 pilot 与 014404 批次保留为诊断记录；最终权威为 015120 批次，不对旧证据做追溯改写。

## 尚未完成

此证据只覆盖选定登记/取消边界及回调复制、断连。TCC 写入进程崩溃、数据库崩溃/重启、完整故障/拓扑矩阵、动态 action-context 报告及生产权限/保留/清理/负载/运维仍需实施验收；AT/XA 剩余门禁及 Phase 5–7 也未完成。完整 TCC 门禁和总计划继续保持未完成。
