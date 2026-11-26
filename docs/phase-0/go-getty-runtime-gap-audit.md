# Getty 帧候选之后的运行时缺口

只读核对固定 Go 提交 `82570d905919c1c66bb025a3a5979f25390de498`。本文不把源码推断记为运行时通过，不修改现有 WP 验收或初始化规格体系。

0004 的验证边界是实际 `RpcPackageHandler.Read/Write`，以及调用它的自定义 TCP 探针。它没有启动原 SDK 的 dubbo-getty event loop。即使实际 Java TC 往返通过，仍必须核对后续各层：

```mermaid
flowchart LR
  S[SessionManager.startGettyClient] --> E[dubbo-getty event loop]
  E --> C[setSessionConfig 选择 handler]
  C --> F[Read/Write 帧和载荷]
  F --> L[gettyClientHandler.OnMessage]
  L --> P[clientOnResponseProcessor]
  P --> M[按原请求 ID 完成 future]
  M --> A[调用方结果与错误]
```

已核对的源码事实：

| 位置 | 固定源码行为 | 下一步验证 |
|---|---|---|
| `pkg/remoting/getty/session_manager.go` | `startGettyClient` 启动 `RunEventLoop(g.newSession)`；`setSessionConfig` 直接设置原 `rpcPkgHandler` | 明确选择适配的生命周期，验证新连接和重连使用同一选项，保留默认行为 |
| `pkg/remoting/getty/readwriter.go` | 原单例是空配置 handler；0004 的显式字段并未自动修改该单例 | 在独立候选测试实际 session 安装，避免以自定义 TCP 探针代替 |
| `pkg/remoting/getty/listener.go` | `OnMessage` 按正文 `GetTypeCode()` 交给 processor；非类型化正文只记录错误 | 真实回调和错误关闭路径、心跳、未注册消息及超时 |
| `pkg/remoting/processor/client/client_on_response_processor.go` | 原 Seata 分支注册了60 processor；合并结果按缓存请求的 `Msgs` 迭代，并直接索引 `MsgIds[i]` 与 `mergedResult.Msgs[i]` | 对结果/关联ID数量失配做行为测试；源码未在索引前验证这些长度，不能由 decoder 的容器长度检查推断安全 |
| 同上及 `getty_client.go` | 完成 future 后清理，超时归属由已有实现处理 | 正常、重复、晚到、错误、连接关闭及响应丢失时不误完成其他请求、不泄漏 future、不重发事务 |

上表最后两项是必须验证的端到端行为；当前只读索引检查不是已执行的失败重现。121批量响应、压缩、其他 serializer、V2，以及完整 RM/Undo 不属于0004已认证范围。原始版本与默认 registry 的59/60缺口仍然开放。

对应当前帧阶段：[计划](go-java-v1-getty-profile-plan.md)、[验证工具](../../tools/go-java-v1-getty-profile/README.md)。正式 Rust WP-01 与完整 Phase0–7 目标保持不变。

## SDK 会话与关联候选增量（2026-11-12）

0005显式配置接入原SDK InitGetty/SessionManager/dubbo-getty loop/listener/processor/future；新鲜Java475/Rust117/Go235（七包、222继承+13新增）和Python9通过，继承getty6/merge7/exception6/length6/control5另行通过，并发批响应重复20次通过。原掉响应、部分完成/越界、错误信封/类型、并发混合批结果及配置冲突均有失败/修复证据。

真实SDK17请求/17实际TC响应、两连接注册/心跳、11次重复并合并发送、5次晚到原响应和6种注入得到5次类型化未知结果；两个原XID回滚后Finished，零事务重发，受检future清理。原Getty EOF不自动重连已有独立失败与带owner/registry校验的候选修复，默认路径未自动启用。[证据](evidence/local-go-java-sdk-profile/README.md)。

新发现[JAVA-V1-PREFACE-001](java-v1-preface-fragmentation.md)保持OPEN：固定TC拒绝首个magic字节单独到达，实际负向连接/源码已归档。SDK场景只强制TC→SDK分片，不能据此声称任意双向分片兼容。原版本默认缺口、完整客户端初始化/自动聚合、在途故障恢复、RM/真实Undo、生产Rust及WP-00仍未完成；待确认的冻结提案不自动生效。

## SDK 在途结果候选增量（2026-11-12）

对应[在途验收计划](go-java-v1-inflight-plan.md)及已接受ADR-0008。0006只在显式同步路径区分未发送与结果未知，绑定真实会话/确切future，保留TC明确响应；完整Go254（235继承+19新增）、Rust117/Python5及静态检查通过，Java617明确继承已认证父证据，本轮未重跑。

原0005同一19项行为对照、处理器暂停/ID复用、binder查找窗口和并发代理分片均保留失败证据。统一短锁及入站对象快照修复归属竞态，合法TC→RM同ID请求不误丢弃；旧子项批次新增成功/整批未知/新所有者保护控制。代理整帧分片按连接/方向加锁。

最终实际SDK四连接均注册，19请求/19实际TC响应，Begin4/Commit1/Rollback3、查询7；三次响应丢失断连和一次自然20秒超时均返回可识别UnknownOutcome，晚到原响应不复活等待。Commit状态实际8→15，只查询不重发事务，四原XID最终核对/清理。[归档证据](evidence/local-go-java-inflight/README.md)。

原版/default及前五补丁、旧副本与原Java JAR保持不变。未知Begin的XID来自测试TC观察者，不代表通用Begin幂等恢复；完整业务/异步客户端、自动聚合发送策略、真实RM/Undo、TLS、生产Rust、完整WP-00与Phase1–7仍未完成，分阶段冻结提案仍待确认。下一步继续完整客户端初始化/资源注册与自动聚合，以及FILE/RAFT恢复和真实补偿缺口，不重复把选定同步链路计为全部交付。

## 完整Go SDK与真实MySQL AT增量（2026-11-13）

按[真实AT计划](go-java-v1-real-at-plan.md)完成三场景新鲜验证：原client.InitPath/TM/RM/SQL代理正常提交和补偿；阶段一后断连及真实资源重注册，重复回滚生成status1防悬挂记录且业务不再改变；原SDK进程退出、MySQL重启后原XID/BranchId/完整Undo字节不变，新PID补偿原事务且不发第二Begin。TC终态查询和独立业务/Undo SQL分别核对。[完整证据](evidence/local-go-java-real-at/README.md)。

Python16通过，759输入导入前冻结，Java覆盖类固定清单/10class字节链及每场景前后/实际加载来源核对。原Go714成员及既有24候选/13辅助未改，Go254/Rust117/Java617明确继承已认证父证据，本轮未重跑其单元测试。审查发现的恢复代理/关联、空SQL快照和class哈希缺口经真实负向测试后修复；期望的prepare断连单独分类，其他错误不放过。

这些是参考Go真实资源证据，不是生产Rust。正常Begin/Registered持久重放不能替代RetryTimeout16/17的008–009完整验收；全SQL/故障原子性矩阵、混合语言共享资源、生产认证及完整WP-00/Phase1–7仍开放。新记录[GO-AT-RESOURCE-ID-001](go-at-resource-identity-divergence.md)：原Go标识含DSN凭据且命名不同，不能据单Go RM成功推断Java/Go/Rust共享锁安全。后续推进该边界及真实重试超时恢复，既有冻结提案仍待确认。
