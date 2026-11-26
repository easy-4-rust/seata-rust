# 正式 Rust RM 与原 Java TC 互操作

[最终014916](run_20261114_014916_69ccac54/result.json)六例通过：正常、重复、断连各提交/回滚。原 TC 签发身份、原 RM 回调/回包、独立 TM 观察及 TC 原日志相互核对，所有资源动作各一次，原事务和分支注册均无自动重发。[完整范围与合并验收](../rm-gate/README.md)。

6个原JavaTC、12个双端观察代理、54个关键类来源已归档；全部自有服务已清理。2075输入/2169成员，catalog SHA256=`c4763b18ce8143b932683d4a4c95a91fb57b6e7dfd3fabd0cd26147d3d574da2`。Java pin=`cba73e180fd8896a99df6415b859d275122feaa7`，原JAR SHA256=`aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`。没有重新编译Java或执行新JUnit。

其中断连回滚保留首次调用未知，记录迟到原Failed/RollbackRetrying=5；TC重试获得原分支Rollbacked=8。Finished=15仅是清理观察，不能覆盖原未知。具体超时时刻没有独立追踪证据，不作该项推断。

业务资源为测试夹具；SQL/Undo原子性、生产认证、完整V1与后续Phase未完成。原Go默认兼容/WP00-R继续开放，外部TC风险未接受，共享锁暂缓。历史较弱或失败运行只作诊断，不能替代最终六例与合并校验。
