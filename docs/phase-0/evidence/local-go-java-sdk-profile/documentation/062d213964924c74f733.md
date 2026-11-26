# 原SDK Getty会话候选验证

规格：[SDK增量计划](../../docs/phase-0/go-java-v1-sdk-profile-plan.md)。0005依赖0001–0004，默认关闭 `java-v1-profile`；显式选择捕获配置快照，在原SDK InitGetty/SessionManager/dubbo-getty loop/OnMessage/response processor/future路径工作，不用自定义TCP客户端替代SDK。

```powershell
$env:PYTHONDONTWRITEBYTECODE='1'
python tools/go-java-v1-sdk-profile/run.py
python tools/go-java-v1-sdk-profile/test_validation.py
```

工具沿用已安装的固定Java/Go构建环境，不初始化、安装、切换分支或修改原参考源码/JAR。所有开发文件与运行目录在D:/E:；28091/28092必须空闲。继承Java475/Rust117、全部0001–0004字节/字段验证，运行受影响七个Go包与13项新增测试，再执行真实SDK会话。

真实会话观察范围：17请求/17实际TC响应、两条连接各自注册/心跳、11次重复并合并发送的响应、五次错误后晚到正确响应、六种代理注入、五次类型化未知结果、两个原始XID回滚后Finished、零事务重发、受检future清理。TCP代理保存原帧与注入帧；错误后等待SDK写出已观察类型化错误的标记，才发送晚到原响应。

固定Getty把EOF视为不自动重连；显式候选捕获会话所属entry，只条件替换该代际，并在重建前后确认registry可用性。旧回调不能关闭新client，也不能复活已移除的地址。默认路径保留原重连行为。配置压缩冲突在初始化前拒绝，不进入原库的立即重试连接循环。

MessageFuture使用同一CAS发布Response和Err。合并分发串行领取整批关联，先验证所有字段再交付；不一致时只向已知关联项发布同一类型化未知错误，绝不部分成功。缺失ID不能用来猜测其他future。已支持的普通响应也验证信封和正文类型。原SDK仍保留通用超时/写错误；本候选的未知响应错误不能等同完整生产Rust错误API。

`*_test.go`绑定仅进入测试，`native_*`先证明原processor掉响应/部分完成/越界和一个正常控制；不依赖新API的运行时flag测试证明原选择缺失。补丁生成与验证分离，实际文件同时匹配补丁清单和格式化后的可编辑实现，检查所有714原成员及完整Go路径集合。

已知限制：[JAVA-V1-PREFACE-001](../../docs/phase-0/java-v1-preface-fragmentation.md)保持OPEN。固定Java TC会拒绝首个magic字节单独到达；本SDK场景仅强制TC→SDK分片，并独立保存原TC首字节拒绝的负向证据。TCP→TC一次完整write也不保证网络不会分片，不能把代理缓存当作通用修复。

该候选验证SDK remoting层，不认证完整业务客户端初始化、自动聚合策略、在途响应丢失后的全部恢复、RM/真实Undo、认证/TLS、生产Rust或完整WP-00。证据目录：`docs/phase-0/evidence/local-go-java-sdk-profile/`。
