# seata-client

统一拥有 TM/RM 的配置和生命周期。通过 `SeataClient::builder(connection, identity)` 提供同一连接配置与登记身份，再显式调用 `enable_tm()`、`enable_rm(catalog, store)`，以及 `allow_plaintext()` 或启用 `tls` 功能后的 `transport_tls(tls)`。

`build().await` 建立本地拥有者，TC 登记成功仍须查看原管理器连接状态。`transaction_manager()` 和 `resource_manager()` 返回借用；事务、分支、资源目录、恢复权限及只读采样器沿用底层公开 API。AT/XA/TCC 引擎由原 `BranchResource` 接入，正常依赖图不引入数据库或 ORM。

RM 先启动；TM 启动失败时等待 RM 清理。`shutdown().await` 消费入口，依次尝试两方关闭并保留独立错误。丢弃或取消关闭使用底层拥有者 Drop；Pending 日志保留，关闭不会替业务发送 Commit/Rollback。

默认没有传输策略，必须显式选择。两个角色共用 TLS 身份、信任和登记预算，没有单独覆盖或明文回退。错误的 Debug/Display 只输出固定诊断，原错误仅经显式访问器取得。

验收见 [统一入口契约](../../docs/phase-1/client-facade-contract.md)。实际 TCP/mTLS、独占日志和原 Java TC 联试分开记录；该入口的交付不代表 WP07 全部完成。
