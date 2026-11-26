# V1 control envelope baseline

> 2026-11-13 implementation update: a formal Rust workspace now exists. Its27-type payload codec includes59/60 and passes the44 existing Java merge payloads, with explicit configurable tree limits; [formal evidence](../phase-1/evidence/protocol-merge/README.md). The probe-only/no-workspace statements below describe the original evidence epoch. Complete production frame/HeadMap/heartbeat and full WP-01 acceptance remain pending; original Go gaps remain open.

Java authority: `cba73e180fd8896a99df6415b859d275122feaa7`; Go reference: `82570d905919c1c66bb025a3a5979f25390de498`. [Executed evidence](evidence/local-control-envelopes/README.md) and [runner](../../tools/java-control-envelopes/README.md). The original29 payloads and six complete frames remain unchanged.

## Wire mapping

| Family | Outer kind | Serialized body | Correlation |
|---|---:|---|---|
| PING |3| No ordinary body, including no120 type prefix | Outer32-bit request ID |
| PONG |4| No ordinary body | Same outer ID |
| MergedWarpMessage |0 in these samples| Type59, int32 content length, signed short count, typed children, optional int32 child IDs | IDs omitted entirely or exactly one per child |
| MergeResultMessage |1 in these samples| Type60, int32 content length, signed short count, typed result children | Children retain original request order; no child-ID tail |

The merge content length excludes its own four bytes and the top-level two-byte type. Each child begins with its own two-byte type; ordinary children have no additional generic per-child length prefix. Nested request batches carry their own merge length. Java result children must be `AbstractResultMessage` instances. A nullable ordinary payload field is interpreted through its particular Java codec, not a universal empty-string rule.

The optional HeadMap belongs to the frame header. Java encodes null header values with signed length-1 and empty strings with0. Only a normal nonempty `traceId=hb` is included in the cross-language heartbeat samples. Rust's null/empty header tests are source-derived synthetic checks, not additional Go/Java certification.

The test decoder consumes sealed sub-readers, requires full consumption and rejects partial/extra child IDs, requests inside merged results, unknown types, malformed UTF-8/lengths and body bytes on heartbeats. This intentionally avoids copying Java decoder branches that may return a partial object on malformed input. Probe limits are8MiB frame, signed-short header length,1024 children per batch,2048 total nodes and depth8 for encode/decode. These are test-tool resource limits, not a claim that Java imposes the same limits or a frozen production API configuration.

## Executed samples

| Case | Full bytes | Java/Rust | Pinned Go |
|---|---:|---|---|
| HeartbeatPing |16| Field/byte verified | Actual Getty read/write byte-compatible |
| HeartbeatPong |16| Field/byte verified | Same |
| HeartbeatHeadMap |29| Field/byte verified | Same |
| MergeRequestWithIds |50| Two typed children; IDs7/-9 | Missing native codec59 |
| MergeRequestWithoutIds |42| Two children; no ID tail | Missing native codec59 |
| MergeRequestEmpty |24| Zero children/IDs | Missing native codec59 |
| MergeRequestNested |54| Nested request batch; outer child ID42 | Missing native codec59 |
| MergeResult |38| Begin success/XIDa and commit success/status9 | Missing native codec60 |
| MergeResultEmpty |24| Zero result children | Missing native codec60 |

Java's actual encoder/decoder generates the frames, independently checks their fields and re-encodes identical bytes. Rust decodes typed objects and encodes new bytes without retaining an original-byte shortcut. Every truncated prefix, trailing data, hostile header, signed/count/ID shape, recursion/global-budget boundary and every single-byte mutation is checked. Accepted mutations must re-encode exactly without panic.

## GO-V1-MERGE-001 — open native codec gap

The pinned Go `message_apis.go` declares59/60 model types, but `codec.Init()` registers neither codec. Actual `CodecManager.GetCodec` returns nil. Actual Getty reading consumes these valid Java frames and returns an RPC envelope with a nil body; `CodecManager.Encode` returns an empty serialized payload. The six diagnostic cases verify this absence, not interoperability. No custom codec is registered to make the reference appear compatible.

This does not prove every Go application uses batching or that normal individual requests fail. The original29 supported payload samples remain verified. Formal transport/capability design must distinguish supported individual-message traffic from unsupported batch traffic; it must not infer support from a declared model name. Full Go compatibility remains in the overall plan, together with the previously recorded failed-response differences. No upstream implementation, baseline fixture or confirmed acceptance requirement was changed.

## Remaining implementation work

These Rust models live exclusively under the independent probe's `cfg(test)` payload modules. There is still no formal protocol crate or production workspace. Compression, V2, arbitrary peer capabilities, production fuzzing and public error/configuration APIs remain separate implementation and acceptance work. This increment supplies a bounded wire baseline and known-gap evidence, not Phase0–7 completion.

## 显式合并载荷候选增量

0003在独立候选副本中新增JavaV1MessageProfile：原25普通类型、59/60、全250异常语义及有符号/可选字段。新38个Java合并样例与原六个合并样例全部通过，另有29普通载荷回归；Java475/Rust117/Go110及新Python7通过。注册缺省版本用WireVersion选项及Java getter上下文分开表达，null状态不等同Unknown0；不能把Java缺省版本的再编码说成原字节相同。[API](../../tools/go-java-v1-merge-profile/README.md)，[执行证据](evidence/local-go-java-merge-profile/README.md)。

这是载荷阶段：原Go registry仍无59/60，Getty帧接入尚未实施，完整GO-V1-MERGE-001仍开放；不能把显式入口结果改写为原版兼容。

## Getty 显式帧候选增量（2026-11-12）

0004在独立副本显式注入实际RpcPackageHandler，验证分片、连续帧、错误传播、有序可空头及长度边界。完整新鲜Java475/Rust117/Go181通过，新Python6和继承merge7/exception6/length6/control5通过。326帧中9帧由Java实际编码，317帧由工具封装Java认证载荷，不扩大Java帧认证数。

未修改的Java TC通过注册、Begin/Status/Begin合并请求、两个原始XID的Begin查询/回滚/Finished查询，8请求/8响应、零事务重发；端口和自有进程已清理。实际handler由自定义TCP探针调用，尚未验证dubbo-getty会话循环和原SDK的适配选择/完整业务分发。[证据](evidence/local-go-java-getty-profile/README.md)，[后续运行时缺口](go-getty-runtime-gap-audit.md)。原版/default兼容、生产Rust、RM/Undo和WP-00仍开放；分阶段冻结提案尚未获确认。

## SDK 会话与关联候选增量（2026-11-12）

0005显式配置接入原SDK InitGetty/SessionManager/dubbo-getty loop/listener/processor/future；新鲜Java475/Rust117/Go235（七包、222继承+13新增）和Python9通过，继承getty6/merge7/exception6/length6/control5另行通过，并发批响应重复20次通过。原掉响应、部分完成/越界、错误信封/类型、并发混合批结果及配置冲突均有失败/修复证据。

真实SDK17请求/17实际TC响应、两连接注册/心跳、11次重复并合并发送、5次晚到原响应和6种注入得到5次类型化未知结果；两个原XID回滚后Finished，零事务重发，受检future清理。原Getty EOF不自动重连已有独立失败与带owner/registry校验的候选修复，默认路径未自动启用。[证据](evidence/local-go-java-sdk-profile/README.md)。

新发现[JAVA-V1-PREFACE-001](java-v1-preface-fragmentation.md)保持OPEN：固定TC拒绝首个magic字节单独到达，实际负向连接/源码已归档。SDK场景只强制TC→SDK分片，不能据此声称任意双向分片兼容。原版本默认缺口、完整客户端初始化/自动聚合、在途故障恢复、RM/真实Undo、生产Rust及WP-00仍未完成；待确认的冻结提案不自动生效。
