# FILE 自然提交超时与真实 AT/TCC 持久重放

延续 REC-TIMEOUT-008、ADR-0008、已认证 DB 混合提交和 FILE 自然17证据。现有 Markdown/ADR 是规格事实源，不调整 WP00 前置或正式 Rust 范围。原 Java JAR 与固定 Go SDK 使用已有独立兼容候选；TC 会话/锁使用同步 FILE，MySQL仅保存 accounts、undo_log、commit_account、commit_reservation。

完整SDK真实注册 TCC/AT 分支并完成本地业务/Undo；TM仅一次Begin/Commit。代理阻断实际二阶段Commit，让原TC调度器自然进入16；纯AT的AsyncCommitting、手动改状态或合成日志不能替代此证据。

停止TC/SDK、重启MySQL，以原FILE journal启动新TC/SDK：原XID、两BranchId、AT完整Undo及TCC预留必须保留，提交锁可用，16不能自动回调。实际人工重试中，原TCC业务确认10后成功回复丢失，原AT真实清理Undo并答复；TC仍16且仅原TCC分支保留。再次停止/重启三方并人工重放原TCC：余额仍900、Undo空、TCC确认10且效果计数仍1，TC会话/分支清理。

每次停机journal与下次启动前的原字节分别绑定；允许首次人工提交移除AT分支后journal正常追加，不能误要求整个生命周期字节不变。独立SQL、控制台及仅101/15/21原帧核对业务与TC结果；完整SDK注册、方向/外层类型、请求ID、原Begin/Commit目标XID、真实回调/丢包与人工操作顺序须一致。

先以缺少FILE校验的既有归一化验证器运行负向控制取得RED，再补严格存储验证；实际运行后用原始证据破坏测试覆盖混合分支、幂等、提前回调、错误帧/身份、配置、日志/PID及journal恢复。冻结输入和父证据，逐轮核制品、配置、类字节/实际来源，完整性与清理通过后才能标记成功。只启动和停止自有D/E目录/端口/容器。

范围仅此FILE自然16链路；不表示SDK自动保障任意TCC业务、完整TCC Fence、额外配置、RAFT真实业务、全SQL/并发/崩溃矩阵或正式Rust已验收。未打补丁参考缺陷及完整WP00/Phase1–7仍分别保持原状态。
