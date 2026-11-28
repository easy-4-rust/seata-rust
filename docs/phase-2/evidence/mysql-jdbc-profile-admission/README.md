# 完整 JDBC 配置的登记前准入（2026-11-28）

新增可选 `MySqlOptions::require_at_jdbc_profile_binding()`，在原业务事务中核对原 JDBC 资源、TINYINT 位与完整稳定配置码，适用于已接入该入口的 Rust 写入器。配置 3/5 和 4/6 的 TINYINT 位相同，完整码检查补齐了仅布尔位无法区分的表示差异。原默认行为、编号 1–6、旧表和旧记录不变。

这是 WP05 写入器准入的一项生产守卫。Java/其他客户端接入、所有写入器的全局锁协作、账号权限及生产部署准入仍开放，不由本守卫替代。

## 使用与事务边界

部署方排空旧写入器与未完成分支后，应用既有 005/011 和新增 [021 迁移](../../../../crates/seata_db_mysql/migrations/021_seata_rs_at_jdbc_profile_binding.sql)。021 保留旧行 `jdbc_profile=NULL`，不从 TINYINT 位猜测模式；维护方须显式填写相互一致的完整码与 TINYINT 位。应用账号与维护权限仍须按部署策略单独约束。

```rust
let strict_options = options.require_at_jdbc_profile_binding();
```

事务连接池或准备管理器任一端启用均会生效，另一端宽配置不能取消要求。登记前在原事务上取得绑定行共享锁和 MDL，核对 InnoDB 基表及当前引擎、原资源 ID、TINYINT 位和完整码。NULL、缺列或不同编号使准备失败，本地业务回滚；BranchRegister 不发送。共享锁保留到该原事务实际结束。第一阶段提交后变更绑定仍要求运维确认所有旧全局分支已排空，本守卫不自动执行配置切换。

旧 `require_at_resource_binding` 和 `require_at_tinyint_profile_binding` 不读取完整码列，原表仍可使用。完整码严格入口不会自动建表、填码、签发准入或改写原 Undo。

## 实际验证

权威现场 `java_20261128_120508_5f38fa21`，catalog SHA-256：`4c866f209dd2a222763b0f1165b1e2cb3f98810ed5a097c5724c5a0e94d379e8`。原 Java 提交、TC JAR、Connector/J、Jackson/JDK 依赖及 MySQL 8.4.13 都按既有固定来源核验；原 TC 未修改，使用既有测试身份检查钩子。

| 入口 | 实际范围 |
|---|---|
| 事务池启用，管理器不启用完整码守卫 | Rust 1.99，配置 5/6 × 提交/回滚，共 4 笔全局事务 |
| 管理器启用，事务池不启用完整码守卫 | Rust 1.85，同样 4 笔全局事务 |
| 私有绑定函数的真实数据库矩阵 | 每个 Rust 版本覆盖六码：6 个相同码接受、30 个不匹配拒绝 |

每笔真实全局事务先对普通 INT 表尝试其余五个配置，避免被 DATE/TIME 编码限制提前拒绝。40 次错误配置准备全部得到明确的绑定错误，业务值回滚，独立数据库观察无分支；原始帧证明只发送了正确配置那一次 BranchRegister。随后正确配置完成六条正式 TIME/DATE DML、原子 Undo/SHA、实际 TC 回调和最终业务/控制/TC 清理，共 8 笔全局事务、48 条接受的业务 DML。Java 原元数据/行图/parser 与 Rust 的原格式比较保持一致。

两个真实 MySQL 矩阵另验证：005 旧表不需要新增列；011 旧入口不需要完整码；严格入口缺列返回 1054，021 不自动填码且 NULL 被拒绝，未知码 7 被服务器 3819 约束拒绝。独立 MySQL 连接尝试把绑定由 5 改为 3，实际 `performance_schema.data_lock_waits` 证明它在等待原事务的共享锁；原事务回滚后更新才完成。

独立核验器 `validate_jdbc_profile_admission.py` 检查原始目录、来源、当前源码/输入 ZIP、实际协议与回调、物理结果、两条启用入口和六码/行锁矩阵，218 次证据削弱被拒绝。`java_20261128_120202_59837c79` 是此前单例探索，不能代替最终八组与双版本结果。

Rust 1.99/1.85 全工作区各 824 项通过、409 项忽略、192 个成功汇总（含 rustdoc），严格 Clippy 均通过。工作区回归/严格 Clippy 的最终结果见 `regression_1.99_*`、`regression_1.85_*`。`tdd_20261128` 保留 API 缺失的初次红灯及五项选项测试绿灯；真实数据库与 TC 现场为独立验收证据。忽略项不计为通过。

`build-artifact-compression_20261128.json` 记录仅在核实构建进程与目标程序均已退出后，对本任务两个构建目录的 320 个产物执行无损 NTFS 压缩；每个文件压缩前后逻辑 SHA 相同，没有删除或搬移文件。该记录是历史压缩操作，不把后续重编译的缓存内容当作静态证据。

完整 WP05、WP07、V1 及后续 Phase 5–7 继续开放。下一步继续其他写入器接入与权限/全局锁准入，以及尚未覆盖的正式 SQL 能力；本轮不宣称所有写入器已被强制接入。

最终封存 `validation_20261128_final`，catalog SHA-256：`fd8872664a3e19747675810ef946e7913119306482ab9e8cc3f87327890d1897`。最终检查器 `validate_jdbc_profile_admission_checkpoint.py` 同时重算原始现场、六码/行锁、两条入口和双 Rust 当前源码绑定。
