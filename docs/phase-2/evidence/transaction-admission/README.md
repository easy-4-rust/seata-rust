# WP07 应用事务准入与排空

事实源：[V1 验收清单](../../../phase-0/v1-implementation-acceptance-checklist.md)与[停机契约](../../v1-feature-matrix-and-recovery-guide.md)。本增量不关闭 WP07 总项，不更改原 TC 结果分类与 Undo 格式。

TransactionAdmission 共享容量和关闭边界，try_enter 返回作用域持有的独占许可。drain 关闭准入并等待许可全部释放，超时返回观察时数量且保留关闭状态。许可 Drop 先归还容量再通知全部等待者；登记通知早于读数量。取消 drain 不重开入口，取消或 panic 释放许可而不自动发送任何事务结束请求。

7 项新增验收：非法容量与容量耗尽/共享关闭；超时后原许可仍有效并可继续排空；两个已挂起排空任务只在最后许可释放后完成；取消排空仍保留关闭与许可；业务取消和 panic 释放许可；32 个并行入口与 close 的竞态；真实 TCP TM 在 Commit 响应屏障期间拒绝新业务且排空超时，响应放行后原作用域完成、排空成功、关闭 TM 并观察 socket EOF。TCP 对端是受控原生报文夹具，该项不是原 Java TC 或部署信号联试。

[缺 API 的首次失败](red.log)；[删掉通知的变异失败](no-notify-red.log)。后者仅把 TransactionPermit::drop 的 self.released.notify_waiters() 删除，运行 multiple_drainers_wait_until_the_last_admitted_scope_exits，已挂起任务无法在一秒期限内唤醒；随后还原生产源码。

Rust 1.99 和 1.85 各通过 47 项 TM 组件测试及 1 项编译文档示例，7 项需外部环境的已有用例保持 ignored。示例把业务构造放在准入后，排空超时保留所有者槽位。没有用 ignored 或仅编译作为完成证据。

- [1.99 全部 TM 测试](1.99-green.log)
- [1.85 全部 TM 测试](1.85-green.log)
- [最终示例 1.99](1.99-doc-final.log) / [最终示例 1.85](1.85-doc-final.log)
- [1.99 全目标严格 Clippy](1.99-clippy.log)
- [1.85 全目标严格 Clippy](1.85-clippy.log)

命令为 cargo +1.99.0 / +1.85.0 test -p seata-tm --offline --locked，以及 clippy -p seata-tm --all-targets --offline --locked -- -D warnings。缓存、临时目录和构建目标均在 D/E 盘，未改变依赖或锁文件。格式检查单独记录于 format.log。

应用应持有许可到业务作用域结束或持久交接完成。许可释放不能证明分布式事务已经结束；Unknown 仍独立保存原身份。直接 TM 入口没有被隐式拦截，生产应用须统一入口并验证所有路径。OS 信号、真实服务停机与完整部署验收尚未完成。
