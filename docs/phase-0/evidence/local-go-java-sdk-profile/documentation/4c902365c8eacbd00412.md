# 原SDK Getty会话候选证据 — 2026-11-12

主运行 `run_20261112_210601_f0a377f3`，完成于 `2026-11-12T21:13:54.531210+08:00`。[规格](../../go-java-v1-sdk-profile-plan.md)、[复现](../../../../tools/go-java-v1-sdk-profile/README.md)、[完整清单](manifest.json)。

| 验证 | 实际证据 |
|---|---|
| Java | 475项、16套，零失败/错误/跳过；2122原生产文件不变 |
| Rust独立探针 | 117项；Clippy/格式通过，仍非正式组件 |
| Go | 七包235项：222继承+13新增；20次并发批响应重复验证 |
| Python | 新9；getty6/merge7/exception6/length6/control5另行通过；18个Python输入在正式导入前冻结 |
| 帧继承 | 326：9个实际Java帧、317个工具封装的Java认证载荷，分组未混淆 |
| 实际SDK | 原InitGetty/SessionManager/dubbo-getty loop/OnMessage/response processor/future；17请求/17实际TC响应，两连接均注册/心跳 |
| 故障注入 | 11次重复并合并发送、5次晚到原响应、6种注入；5次类型化未知结果，错误外层ID被忽略 |
| 原事务 | 两个原XID跨连接仍Begin，均回滚后Finished；实际TC各Begin只执行一次；零事务重发 |
| 清理 | 实际使用请求ID范围内future/cache清空，自有TC/SDK结束，28091/28092释放 |
| 来源 | 全部714原Go成员、0001–0005、全部执行测试/工具和可编辑模板逐一核对；上一阶段副本未修改 |

修复前实际processor丢弃整批响应，旧子请求路径可能部分成功后越界；原配置缺少显式选项。新增测试另行复现并修复并发重复响应混合结果、错误未传播、type4/6漏分发、普通响应错误信封及初始压缩配置冲突。MessageFuture用同一CAS发布结果/错误，整批关联只由一个处理过程分发。配置快照、旧owner和registry移除期间不复活连接有回归。

真实EOF失败尝试与修复、首次TC首字节分片失败、初期代理把SDK正常退出RST当作异常的失败尝试均保留在preliminary目录；不是认证成功运行。最终运行另有独立直接TC首字节负向复现及原探测器源码/哈希。

**JAVA-V1-PREFACE-001仍OPEN**：原TC拒绝首个magic字节单独到达。[问题记录](../../java-v1-preface-fragmentation.md)。本SDK场景只故意拆分TC→SDK方向；向TC整帧write并不保证网络不分片，不能称为通用规避或双向任意分片兼容。

TC使用FILE、关闭认证；本轮覆盖SDK remoting选定路径，不认证完整业务客户端初始化、自动聚合、全部在途故障恢复、真实RM/Undo、认证/TLS、生产Rust或完整WP-00。普通超时/写错误仍有原SDK语义，不将新响应错误类冒充完整生产错误API。原版/default缺口保持开放，分阶段冻结提案仍待确认。

0005 SHA256 `152546ea2847e04c78abc677266dca6029768780db8f815cb1caaab760b8c6bd`；原JAR SHA256 `aef2e8386c97deb78b45c6ad662ad601447def6f5c23537b7d5b1a314654c5d5`。未提交、未推送、未切换分支。
