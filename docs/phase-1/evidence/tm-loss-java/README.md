# 正式TM对原JavaTC的真实响应丢失

[最终224303](run_20261113_224303_f22324e1/result.json)：175输入、216成员、一个实际正式client binary，catalog SHA256=`76f1949973528f422db0a5b9fa583d172f47eabcff935736ba7031ecd2824f63`。三个独立原TC＋三个只丢一条完整成功响应的代理实际执行并清理；固定原pin/JAR/公共认证SPI不变，没有Java编译、覆盖或JUnit。

| 故障 | 正式TM及独立字节证据 |
|---|---|
| Begin | 原TC成功建立事务、代理丢弃唯一响应；`execute`返回无XID的Unknown，业务工厂不构造、不再次Begin。观察器XID不传给客户端；149字节原FILE记录保留在归档，仅证明原请求执行，不声称客户端已恢复这个未知事务。 |
| Commit | scoped业务仅注册一次原分支，锁先占用；成功Commit响应丢失。真实BranchCommit回调按原XID/BranchId/资源/nonce在原连接回包，模拟动作1次。原错误的私有来源凭证恢复同意图，先查询再显式重复一次Commit；查询15、重复响应15，`unknown_retained=true`。未转Rollback，锁释放，独立contender回滚确认11。 |
| Rollback | scoped业务明确返回原业务错误；唯一成功Rollback响应丢失。真实BranchRollback原身份回包、动作1次；业务错误和回滚Unknown均保留。凭证恢复原Rollback，查询后只显式重复同操作；15/15仍保留原未知，未转Commit，锁释放，contender明确回滚。 |

使用正式`TransactionManager::execute/recover_unknown`，不是以历史transport-only probe替代TM。恢复前的反向操作及未查询重复都被本地拒绝；代理原始帧证明没有对应请求写出。driver只读模式、端点及预先选择的nonce，不读观察器文件、捕获XID或BranchId。

每场景核对7个实际类来源，共21处：原JAR内部core的RegTm/RegRm/ChannelManager，原JAR classes的DefaultCore/AbstractCore/AbstractAuth，冻结runtime SPI的checker。配置源、三份实际-Dspring.config.location配置、SPI两文件、client binary和JAR在结束时复核，成功只在输入闭包与进程/端口清理后发布。

独立验收器重新读取原始V1字节，验证原连接/signed ID、双向唯一应答、原分支登记→回调/回包身份及终态、业务nonce、锁资源/key及contender。恢复查询必须由丢响应后新发出的15请求产生，16响应位于显式重试前。不能靠事件元数据或客户端PASS伪造提交确认；Finished不证明历史结果。

历史223750三故障功能通过，但复审发现“原丢弃应答另有投递副本”与“旧在途查询冒充恢复”两处误验收路径；两个新测试方法产生四次实际断言RED，修后13方法GREEN，再以新epoch重跑224303。旧support未改，旧归档保留作诊断，不计最终Gate。

范围为TM＋内存模拟RM。没有SQL/Undo、持久幂等或跨进程恢复；公共fixture不等于生产认证。只关闭[WP03组合门禁](../tm-gate/README.md)所要求的TM故障验收，原Go默认兼容/WP00-R、外部TC风险未接受及共享锁延期不变。

复现新现场：`python tools/formal-tm-loss/live.py`。只使用已有D/E来源/缓存，新建归档和runtime，不覆盖旧记录。
