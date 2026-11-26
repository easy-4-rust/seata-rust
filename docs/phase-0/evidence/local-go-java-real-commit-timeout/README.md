# 真实 AT＋TCC 自然提交超时16与持久重放

正式运行：[run_20261113_005014_9352c041](run_20261113_005014_9352c041/summary.json)，[计划](../../go-java-v1-real-commit-timeout-plan.md)。固定Java TC JAR、已认证前导分片覆盖、Go0001–0006完整SDK和真实MySQL；不改写TC状态、不合成Undo或RM成功响应。

| 阶段 | TC状态 / 剩余分支 / 锁 | AT余额 / Undo | TCC预留 / 确认 / 效果计数 |
|---|---|---|---|
| 真实Commit指令被阻断，自然超时 | 16 / AT＋TCC / 0 | 900 / 原字节status0 | 10 / 0 / 0 |
| TC、SDK及MySQL首次重启 | 16 / AT＋TCC / 0 | 900 / 原字节status0 | 10 / 0 / 0 |
| 实际人工重试，仅丢TCC真实成功响应 | 16 / 仅原TCC / 0 | 900 / 已清理 | 0 / 10 / 1 |
| TC、SDK及MySQL再次重启 | 16 / 仅原TCC / 0 | 900 / 空 | 0 / 10 / 1 |
| 再次人工重试原TCC | 原记录已删除 / 0 / 0 | 900 / 空 | 0 / 10 / 1 |

原TM只有一个Begin/Commit，无Rollback和目标事务重发。三套TC/完整SDK进程、两次数据库重启；所有实际资源注册、原AT/TCC分支类型/资源/请求ID和成功回复一一关联，丢包只属于首次operator的TCC成功回复。首次控制台HTTP200/body code500表示未全部完成，并由原TCC响应等待超时及真实AT提交成功日志解释；最终HTTP200/code200同时经过原始帧和SQL验证。

TCC真实业务以本地事务、原XID/BranchId唯一行和FOR UPDATE行锁保护持久效果，重复committed不再增加业务数量。该幂等属于本夹具业务，不能声称SDK会替任意业务自动实现；未把它当作完整TCC Fence、空回滚/悬挂验收。

本轮27项Python新执行通过；Go254/Rust117/Java617沿用已认证父证据，本轮未重跑。783个输入导入前冻结，含本次schema.sql；每轮原JAR、固定JDBC字节及Driver实际类来源、10个覆盖类字节和5个主类来源均核对。独立审查发现的AT回复被重复TCC回复替代、错误envelope及脚本冻结遗漏已修复；合法不同V1 header的负向用例先失败、后通过，preliminary保留日志与修前源字节。D盘trial虽完成真实业务，但较弱验证器且未冻结新增SQL，不计正式认证。

已知JAVA-METRICS-RETRY-001在原TC重试重入再次观察到，见[日志事件](known-metrics-events.json)，保持OPEN；业务链路通过不代表指标已修复。所有本次容器/进程停止且28091–28093释放。

范围仅覆盖REC-TIMEOUT-008选定混合AT/TCC真实提交重试、原AT Undo清理及TCC跨进程持久幂等。纯AT AsyncCommitting不是本次自然16触发源；FILE/RAFT安全恢复、全SQL/并发崩溃矩阵、混合语言资源标识、生产认证、完整TCC/生产Rust、WP00与Phase1–7仍开放。既有分阶段冻结提案仍待确认。
