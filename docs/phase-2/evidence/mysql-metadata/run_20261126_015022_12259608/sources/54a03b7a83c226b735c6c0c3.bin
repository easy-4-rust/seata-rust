# seata-rm — WP04 资源管理与持久恢复

已实现 `ResourceCatalog`、`BranchResource`、`BranchDispatcher` 及 `FileBranchStore`。本组件按精确资源模式/ID分派，先同步 Pending 再执行资源，未决记录先恢复查询，完成记录同步后才返回完成状态。多个分派器通过共享 Store 的分支租约串行；取消后的磁盘任务继续持有排他权和预算，同租约后续写入也须等旧写真正结束。

文件显式传入，版本头为 `SRMJNL01`，记录绑定递增序号、状态、原生V1分支请求和CRC32。打开时取得合作式整文件独占锁，严格检查身份、方向、迁移、容量和完整性。损坏、残尾、超限或写入/同步错误不被解释为资源成功，也不会自动截断或驱逐去重记录。文件父目录须已存在；日志应由同一资源管理实例独占。

`BranchResource::recover` 必须提供资源级事实。`NotApplied` 要求所有旧尝试均不可能迟到生效；仅查不到效果行不够。`Unknown` 或错误保持 Pending。日志不是数据库效果与Undo的原子提交证明；具体数据库适配器仍待 WP05/06。进程测试使用绑定原始身份的文件效果夹具，仅验证通用恢复契约。

`ResourceManager::builder` 拥有原生登记连接池、有限回调工作任务和关闭顺序；`client()` 返回可克隆的 `ResourceClient`，提供一次性的 `branch_register`、`branch_report`、`lock_query`。返回完整响应；`RmCallError` 区分本地未发送、TC明确Failed、协议失败与发送后未知，并保留原请求。BranchReport的Java可选资源字段保持None，lockable=false是成功观察结果，Failed响应中的true不能覆盖错误。未知分支注册不会补造编号或自动重发。

回调保留原连接、signed请求编号和可空有序头；整批平坦消息先校验，再分派资源。回调总期限在校验前建立，字段按借用先检查边界再复制；矛盾方向和不支持的子消息不会先执行前面的资源。原登记契约不变：初始化期间缓存回调，完整资源修订接受后才转发和发送业务帧。Drop同步关闭所有调用入口；shutdown取消/join资源任务和网络监督器，保留清理panic，并等待有界磁盘排空，失败不能报告完整清理成功。

[运行组件验证](../../docs/phase-1/evidence/rm-runtime/README.md)在Rust1.99/最低1.85各执行45RM＋189已有组件测试及11隔离削弱检查，覆盖动态资源就绪、旧代际晚完成、批次、超时、关闭panic和真实恢复子进程。[完整首期WP04验收](../../docs/phase-1/evidence/rm-gate/README.md)现已通过：另有原JavaTC正常/重复/重连各提交回滚六例、38观察单测、10观察器削弱检查及双版本2项辅助资源测试；真实TC重试不重复资源效果，原未知不被迟到响应或Finished改写。SQL/Undo属于后续WP05/06，尚未实现。规格事实源仍为原 [WP04验收清单](../../docs/phase-0/v1-implementation-acceptance-checklist.md) 与 [RM实施契约](../../docs/phase-0/tm-tc-rm-call-chains.md)。外部TC风险未接受，共享锁问题暂缓，后续全部计划保留。

验证：`cargo test --locked --offline -p seata-rm --all-targets`。测试文件及子进程证据均写入 D 盘专属目录。默认忽略的 `journal_process_child` 是父测试真实启动的子进程入口，不单独计作正式测试。
