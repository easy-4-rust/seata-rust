# 原生影响行响应与空镜像故障验收（2026-11-27）

对应 [WP05 一阶段提交契约](../../mysql-first-phase-commit-contract.md) 中“影响行与镜像保护”的原生驱动故障缺口。生产保护已存在，本轮补齐真实驱动、原 TC 与原物理事务的故障证据，不把此局部验收称为 WP05 或完整 V1 完成。

## 实际路径

固定 MySQL 8.4.13 实际执行零行 UPDATE/DELETE，代理保留服务器原始 OK 包，将其中单字节 affected_rows 从 0 改为 1 再交付 SQLx。包长、序号、insert_id、状态、警告和其余正文均保持原字节。该场景是受控网络故障，不是声称 MySQL 自发产生错误行数。

四种组合为 UPDATE/DELETE 各有无先前未提交 INSERT。默认构建未打开 experimental_changed_commit。客户端每例验证精确镜像错误、后续 DML 为 Poisoned、准备为 Local(Poisoned)。原 RM 已完成资源登记，但原始帧证明没有 BranchRegister、BranchReport、锁查询或二阶段回调，journal 只有版本头。MySQL 原始 client_command 和关联 prepare/execute 证明没有本地 COMMIT，且只有两笔合法先前 INSERT 与四笔目标 DML，后续被拒绝的 INSERT 未发送。

原全局事务只发送一次 Rollback，后续只读查询原 XID，允许 Rollbacking/Rollbacked 后收敛到 Finished。原目标四个物理会话均在服务端消失后，独立 SQL 才核对 global/branch/lock/business/Undo/recovery/attempt 七项全零；不能以暂时查不到业务行替代旧事务已结束。

## 完整原 TC / MySQL 现场

| Rust | 现场 | 结果 |
|---|---|---|
| 1.99 | [2295420e](../java-tc-changed-commit-report/run-affected_image_guard-1.99-2295420e/verified.json) | 四组合通过，原始首轮完整结果 |
| 1.99 | [0a0984bc](../java-tc-changed-commit-report/run-affected_image_guard-1.99-0a0984bc/verified.json) | 恢复原生产字节后四组合再次通过 |
| 1.85 | [0ae04d5f](../java-tc-changed-commit-report/run-affected_image_guard-1.85-0ae04d5f/verified.json) | 四组合通过 |

双版本必需矩阵为八个全局事务，另有 1.99 恢复后的四例重复验证；不是十二种不同组合。每个完整现场保存原 TC/RM/TM 帧、原 MySQL 包、实际改写及交付字节、物理会话退出、七项最终 SQL、默认构建特性和进程清理记录。

将生产 execute 中的影响行保护调用暂时移除，同一原 TC/MySQL 场景第一例返回 Ok(1)，原目标断言实际失败：[失败驱动](guard-mutation-red.log)、[字节摘要与失败目录](guard-mutation-proof.json)。finally 恢复原字节后 [完整重跑通过](guard-restored-green.log)。未改变保护的错误分类、恢复行为或默认公开 API。

## 独立证据校验与回归

[最终重放](independent-replay.json) 绑定三个完整现场的 MySQL/RM/TM 原始字节 SHA-256，每组拒绝 16 种削弱：

- 十项线缆控制：缺改写、实际交付仍为原包、附带状态位改写、伪造原包、增加 COMMIT 事件、语句映射错误、伪造物理会话、响应早于执行、原始 COMMIT 不带事件标签、缺原始 execute。
- 六项原 TC/结果控制：替换原 XID、准备未中毒、残留业务、非空 journal、未完成状态、重复 XID。

首个完整现场现场执行八项控制，随后两个现场现场执行十四项；最终验证器对保留的真实原始输入逐组执行全部十六项，写入各目录 affected-image-replay-controls.json，原文件未覆盖。原 MySQL 帧检查复用独立有界包解码器，不调用故障改写函数来判定改写是否正确。

故障改写函数三项单元测试 [通过](python-unit.log)。原观察器未启用改写的默认通道还通过一次 [原 TC 无变化提交对照](../java-tc-changed-commit-report/run-no_change_commit-1.99-ea18d0df/verified.json)，原 COMMIT OK 正常交付，业务保持原值。

双版本全工作区全功能各 707 项通过、318 项 ignored：[1.99](1.99-workspace.log)、[1.85](1.85-workspace.log)。新增原 TC 用例在普通回归中 ignored，已由上述外部运行器显式执行，不将其他 ignored 计作通过。双版本 MySQL 全目标全功能严格 Clippy：[1.99](1.99-clippy.log)、[1.85](1.85-clippy.log)。

## 首轮诊断与剩余门禁

[2c8c1180](../java-tc-changed-commit-report/run-affected_image_guard-1.99-2c8c1180/client-stdout.log) 首例已触发目标镜像错误，随后测试假设原 TC 立即 Finished 而收到 Rollbacking；驱动也误入无变化提交的屏障分支。修正为原 XID 只读等待，并排除该屏障后完整重跑。此轮不计成功，不把夹具错误称为生产错误。

完整外部写入器锁覆盖、公开有变化提交验收、全部原 Java 用例迁移、WP05/WP06 与后续 Phase 仍开放。

## 复现

在仓库根目录、已具备既有固定镜像和工具链的环境执行：

```powershell
pwsh -NoProfile -File tools/formal-first-phase-commit/run_desktop.ps1 -RustVersion 1.99 -Scenario affected_image_guard
pwsh -NoProfile -File tools/formal-first-phase-commit/run_desktop.ps1 -RustVersion 1.85 -Scenario affected_image_guard
python -B tools/formal-first-phase-commit/affected_image_mutation.py
python -B tools/formal-first-phase-commit/test_affected_image_guard.py
```

变异脚本临时移除保护并在 finally 恢复原字节，然后运行完整恢复对照；不要与其他 Rust 构建或同端口的原 TC 场景并发执行。运行器仅使用自有临时容器和数据库，未安装或升级工具。
