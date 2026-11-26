# 正式TM与固定原JavaTC的六个功能场景

当前[221553运行](run_20261113_221553_243ee496/result.json)：167来源、179成员、一个实际client binary；catalog SHA256=`dcbb3a1ed9c0671dfc9f50a323b84c15356114dc9e72e1f8df0150d8b8831e5d`。原Java pin `cba73e180fd8896a99df6415b859d275122feaa7`及原server JAR SHA256 `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`不变。没有Java源码覆盖、编译或JUnit执行。

正式`seata-tm` API运行六个场景：Begin/Commit；业务错误与Rollback；Required/Supports/Mandatory作为Participant及Never拒绝；RequiresNew/NotSupported挂起恢复；Participant不结束外层事务；本地业务期限届满。实际创建七个不同XID。原TC原始stdout确认7次Begin、4次Commit、3次Rollback，名称nonce、XID及期限与客户端断言对应；一个TC期限为50ms，其余60000ms，与客户端3s传输预算分离。记录七个XID的原9/11等明确应答，再做只读状态查询确认结束，不以Finished代替历史提交证明。

TC会在返回Commit确认后异步清理；驱动最多等待5s，只读观察8/9等已确认结果后的状态，最终七个XID均Finished15。首个215516诊断在六场景均通过后，因错误要求立即Finished而失败，不计正式通过。其后215637/220344/220729功能成功是历史版本；后续恢复来源凭证加入后重新跑221553，最新组合门禁只绑定此版本。

六个实际加载类来源核对：RegTmProcessor/ChannelManager来自原JAR内部core依赖，DefaultCore/AbstractCore/AbstractCheckAuthHandler来自原JAR classes，公共认证SPI checker来自冻结runtime扩展。两实际SPI文件、配置源及实际-Dspring.config.location文件都纳入快照和结束复核；运行成功只在业务/清理/来源闭包检查后发布。自有PID54740与38091端口已清理。

该运行是正式TM的基础Java功能验证：空全局事务，没有RM分支或SQL/Undo，未进行正式TM的Java响应丢失故障注入，完整WP03 Gate保持开放。不能用先前transport驱动的故障测试充当本组件现场完成。公共fixture不等同生产认证；外部TC风险、原Go默认兼容/WP00-R及共享锁延期不改变。

复现：`python tools/formal-tm/live.py`；使用已有D/E来源和缓存，新建独立归档及runtime，不覆写旧证据。

后续[224638组合门禁](../tm-gate/README.md)已用新的正式TM真实Begin/Commit/Rollback丢响应证据关闭WP03 Gate；本页原组件/基础Java的开放状态保留其历史范围，未将旧运行改写为包含故障注入。
