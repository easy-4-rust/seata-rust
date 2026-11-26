# 完整Go SDK与真实MySQL AT证据 — 2026-11-13

主运行 [run_20261113_000911_3c78730f](run_20261113_000911_3c78730f/summary.json)，完成于 `2026-11-13T00:10:28.715474+08:00`。[计划](../../go-java-v1-real-at-plan.md)、[复现](../../../../tools/go-java-v1-real-at/README.md)、[清单](manifest.json)。

| 场景 | 实际结果 |
|---|---|
| normal | 原client.InitPath/TM/RM/SQL代理；业务1000→900、原Undo落库，真实TC/RM回滚后1000；再提交至1025并清理原Undo |
| reconnect | 阶段一后强制断连；两连接，真实资源重注册后才收到原分支回调；业务恢复1000；重复原TC回滚帧得到原身份的真实RM回复，业务保持1000、持久防悬挂记录status1；再正常提交至1025 |
| recovery | 原SDK进程阶段一后os.Exit跳过全局结束；MySQL实际重启，独立SQL与新SDK同时证明原XID/BranchId/Undo字节不变；新PID使用原事务补偿至1000、Undo清理，未发第二Begin |
| TC状态 | 各原事务另行查询至Finished15；数据库结果独立核对，不能单凭Finished认定补偿 |
| 验证器 | Python16通过；坏SQL/缺RM注册/错误ID/晚注册/代理错误及class篡改均须拒绝 |
| 来源 | 759输入导入前冻结；714原Go成员/24候选/13辅助不变；固定MySQL镜像、二进制/依赖build-info、类字节链及5实际加载来源均记录 |
| 继承 | Go254/Rust117/Java617沿用已认证父运行并核对输入/摘要；本轮明确未重新执行其单元测试 |
| 清理 | 三个专有容器均停止，TC/SDK结束，28091/28092/28093释放；D盘原数据保留 |

复用了真实AT实现及持久Undo，没有手工修改业务行来模拟补偿、合成Undo或伪造RM二阶段响应。恢复前后独立SQL必须成功且非空；XID/BranchId、status0、完整blob十六进制/哈希均绑定。恢复后的103→104须先于回调，5→6按原连接和ID验证；正常3→4亦如此。prepare的预期EOF/reset单独记录，不混入代理errors，也不豁免恢复阶段错误。

原首轮正式记录`run_20261113_000011_cd2eb5bd`的业务与数据核对正常，但审查揭示三处验证闭包缺口，不作为最终强验证版本。六项负向测试真实RED、修复后16项GREEN均有日志。[审查](review.json)。更早quick尝试只作诊断，未冻结所有输入，不冒充最终认证。

恢复范围是原TC仍运行时的Begin/Registered正常持久重放；**没有关闭RetryTimeout16/17的REC-TIMEOUT-008–009完整验收**，也未认证全SQL/故障原子性矩阵、跨语言共享资源、生产Rust或完整WP-00/Phase1–7。[Go资源标识及凭据差异](../../go-at-resource-identity-divergence.md)保持OPEN，测试中原样记录的锁键为`ACCOUNTS:1;`，没有规范化后冒充Java相等。

原JAR SHA256 `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`；MySQL镜像 `sha256:a81d043930e51745522bd5c430961cf8286f0a23f0a9a2a6e0108f7ea71b0902`。未修改原版、未提交/推送/切换分支，冻结阶段调整仍待确认。
