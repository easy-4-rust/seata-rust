# 重叠旧回调和迟到失败的保留资料重放

来源：[1.85 初轮原始运行](../../java-tc-changed-commit-report/run-rollback_commit_in_flight_shutdown-1.85-0808f6ca/process-cleanup.json)。原客户端测试成功退出；旧验收脚本随后因假定一个旧回调而失败，没有执行外部最终数据库计数。此目录复制该次运行保留的原始帧、MySQL 事件、前后物理会话 SQL 快照、已同步分支日志及客户端观测，不是新的现场执行。

最终检查器按原始帧核对两个旧请求、一个新会话请求、一次成功回包，以及一次补偿、Undo 删除、围栏和终态写入。原 Failed/RollbackRetrying 在请求后 15039.3333ms 到达，客户端 10 秒期限已保持 Unknown，随后只读 Finished。replay-verified.json 只说明这些保留资料的断言通过；不替代缺少的外部最终数据库计数。

late-failed-reply-controls.json 拒绝提前失败回包、错误终态、异身份和把已观察结果说成未知。完整范围与四组最终现场见 [上级验收说明](../README.md)。
