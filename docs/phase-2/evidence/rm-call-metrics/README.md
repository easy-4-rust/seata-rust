# RM 出站调用计数验收（2026-11-27）

规格事实源为 [WP07 观测契约](../../wp07-observability-contract.md) 和 [V1 清单](../../../phase-0/v1-implementation-acceptance-checklist.md)。此增量不关闭 WP05–07 或完整计划。

RmCallMetricsSampler 按管理器及全部客户端克隆共享三种固定操作的计数；与日志作用域的 RmMetricsSampler 分离。注册回执仅计一次，有效 LockQuery false 属于 observed；未知不重试，取消不证明未发送。只读采样器不持有池或发送能力。认证 HTTP 新增固定 24 条序列，无业务身份标签；关闭后保留客户端仍可能产生本地拒绝和未发送计数。

## 实际验证

- 缺 API 的 [RED](red.log) 与缺 HTTP 导出入口的 [RED](export-red.log) 已保留。
- 五项真实 TCP 用例覆盖共享克隆、单次回执、可选字段、LockQuery false、明确 Failed、无效分支号、响应丢失、取消、未发送及关闭后调用。另有并发饱和边界、真实 Bearer HTTP 与接口示例。[1.99 目标结果](target-green.log)、[1.85 目标结果](1.85-target.log)。
- 将回执内部调用改回公开分支注册接口后，观察数错误为 2，目标测试实际失败：[变异失败](double-receipt-mutation-red.log)。恢复原字节后同一测试 [通过](double-receipt-restored-green.log)。
- 双版本全工作区全功能各 **705 passed / 0 failed / 314 ignored**：[1.99](1.99-workspace.log)、[1.85](1.85-workspace.log)。ignored 不算通过，以下原 TC 现场是额外显式执行。
- RM、observability、MySQL 全目标全功能严格 Clippy 双版本通过：[1.99](1.99-clippy.log)、[1.85](1.85-clippy.log)。

## 固定原 Java TC 与真实 MySQL

沿用既有固定 TC/JAR/ConnectorJ 和 MySQL 8.4.13 镜像。每组原始认证帧、客户端结果、外部数据库最终核对与进程清理均保留；以下六组都有 verified.json、stopped=true 和五个反例拒绝结果。

| Rust | 场景 | 完整现场 |
|---|---|---|
| 1.99 | rm_call_rejected | [run-rm_call_rejected-1.99-48bd7444](../java-tc-changed-commit-report/run-rm_call_rejected-1.99-48bd7444/verified.json) |
| 1.99 | commit | [run-commit-1.99-fa61b8b1](../java-tc-changed-commit-report/run-commit-1.99-fa61b8b1/verified.json) |
| 1.99 | commit_report_loss | [run-commit_report_loss-1.99-243f248f](../java-tc-changed-commit-report/run-commit_report_loss-1.99-243f248f/verified.json) |
| 1.85 | rm_call_rejected | [run-rm_call_rejected-1.85-be422923](../java-tc-changed-commit-report/run-rm_call_rejected-1.85-be422923/verified.json) |
| 1.85 | commit | [run-commit-1.85-e25b7119](../java-tc-changed-commit-report/run-commit-1.85-e25b7119/verified.json) |
| 1.85 | commit_report_loss | [run-commit_report_loss-1.85-40a3340a](../java-tc-changed-commit-report/run-commit_report_loss-1.85-40a3340a/verified.json) |


commit 的 BranchRegister 与 BranchReport 各 observed=1；commit_report_loss 的原 TC 成功报告响应确实产生但代理零投递，BranchReport unknown=1，未重发。rm_call_rejected 使用默认功能、不打开实验写入：真实活动 XID 的 LockQuery observed=1，原事务 Finished 后 TC 明确拒绝注册，BranchRegister tc_rejected=1，无业务修改及新增分支/锁/Undo/控制记录。

计数与原始请求、原连接/请求编号关联和响应实际投递逐字段比对。专用 drop_original_report_reply 仅在对应场景、s2c、kind=14 和严格整数零投递时允许，再复用规范 V1 原始字节校验；其他动作不能借此放宽。每组五个反例为结果错分、重复回执、身份标签、布尔伪装整数及错误投递，共 30 个，全部拒绝。

## 诊断记录

以下不计完整现场通过：

- rm_call_rejected-1.99-5b80abb9：客户端结果正确，验证器的错误投递反例触发 ValueError 未被捕获；修正后重跑。
- rm_call_rejected-1.99-3d664720：计数校验通过，验收脚本将 Java boolean16 误写为单字节，外部最终检查未完成；按正式协议修正后重跑。
- commit_report_loss-1.99-3328352f：通用验证器未识别专用零投递动作，新增严格限定后重跑；不把此轮计为完整成功。

诊断原始目录和对应失败日志一并保留，不覆盖原始证据。MySQL 本地结果全覆盖、重试指标、完整 TM 存活状态、生产采集/告警及部署验收仍开放。
