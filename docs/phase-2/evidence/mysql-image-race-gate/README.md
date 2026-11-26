# WP05 两次镜像读取之间的并发验证

[最终父记录](run_20261114_052627_abb1eddd/result.json)、[只读终核](readonly-review.json)与[部分状态](../../wp05-mysql-image-race.json)补齐[读取契约](../../mysql-row-read-contract.md)的指定竞争时序。当前生产代码不变，INSERT、业务SQL、after image、Undo及完整WP05仍未完成。

最终7955输入/8502成员，catalog SHA256=`cac7531795b6296a541b81c61841e9c2552846b87539079dc7f412225d79311a`。两个Rust版本各实际新执行一个真实MySQL竞争场景，分别保存独立live产物；两份D盘隔离削弱实现均编译并在目标自定义assert失败。原[282工作区/每版19现场及13负控](../mysql-row/README.md)通过223个既有Rust文件（182生产源）、Cargo及依赖原字节绑定继承，本轮没有重跑或混计这些测试。

| Rust | 新执行子记录（各1通过，267成员） | catalog SHA256 |
|---|---|---|
| 1.99 | [run_20261114_052637_1843fe18](../mysql-image-race/run_20261114_052637_1843fe18/result.json) | `7d6922ee2823dbb860e6fb96202cf29050fe223fc6e9764c13503fb49bae14a0` |
| 1.85 | [run_20261114_052703_806f5576](../mysql-image-race/run_20261114_052703_806f5576/result.json) | `600b776e1f798d2c97e3fc31b6b82793768ce9a710ece2c3f7b377f47a589d9d` |

自有loopback非TLS代理收到完整值SELECT的原始COM_STMT_PREPARE后，先保留整个102字节包，再等待显式放行才向真实MySQL转发。四例均为相同原包，SHA256=`0545e948290a7184311184b236fe6134f1276d7726f20c7676ff9ee51b873f3a`，长度98、sequence0、命令0x16；SQL为`SELECT id,payload ... WHERE id>=? ORDER BY id LIMIT1025 FOR UPDATE`（原包保留实际反引号/空白）。这明确晚于长度预检完成、早于完整值查询PREPARE/EXECUTE，无生产钩子或伪造回包。

屏障期间独立连接的BLOB增长至1MiB+1及范围内INSERT均实际1205；放行后读回原一行/一字节，原事务回滚后同两SQL均各成功1行。客户端整链预算显式10秒，包含等待；结果没有依赖固定Sleep猜测持锁。

| 实际削弱 | 真实目标断言失败 |
|---|---|
| 仅去掉长度预检FOR UPDATE，保留完整值查询锁 | 行增长不再被阻塞：`row growth must be blocked before full value query` |
| 配置及profile同改READ COMMITTED，避免先被配置校验拒绝 | 原行增长仍被阻塞，但幻行INSERT不再被阻塞：`phantom insert must be blocked before full value query` |

断言失败不是编译错误或任意驱动失败；父包保留负控完整输入快照、build/live日志、实际二进制及执行源原bytes。该结果符合MySQL官方[隔离级别说明](https://dev.mysql.com/doc/refman/8.4/en/innodb-transaction-isolation-levels.html)，认证范围仅当前8.4.13/lower_case_table_names2实例与该主键范围，不宣称所有索引/条件/服务器配置均已穷尽。

四个自有容器已按image/owner核对并移除，四端口无监听。之后D盘临时目录被清理：历史执行源及exe原路径当前缺失，E盘8502成员/7955快照、两正向和两负向归档均完整，不把归档匹配称为当前运行文件存在。2026-11-15按锁文件及公开C缓存索引核验，从E封存恢复185个archive，并由离线Cargo解包，原6930个archive/源码文件逐SHA一致；[恢复记录](cache-restoration-20261115-result.json)。没有网络下载、工具安装或修改Cargo.lock，也未重建旧负控/旧exe。

本轮没有新生产功能或Java/Go执行，Java迁移仍为九对象PARTIAL/两方法引用/189测试文件库存/七原资产；185锁定归档/148真实编译依赖/6745源继承原证明。源码首次缺测试代理文件的失败仅为编译RED；051700初版已实际通过，052022增加raw原包记录后通过，最终父再次执行双版本及两负控。

原十四项继续0/14、WP06及全部后续Phase开放；外部TC风险未接受、共享锁暂缓、生产TLS/最小权限/完整V1/原生RustTC与发布均未完成。下一步INSERT、业务和后镜像，再接确定性兼容Undo与同事务提交。复现`python tools/formal-mysql-image-race/run.py`，需要已恢复的D缓存，运行输出D/E盘。
