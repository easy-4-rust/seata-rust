# 固定 Go V1 失败响应差异

状态：**3 项开放，不代表互操作通过**。Java 权威快照 `cba73e180fd8896a99df6415b859d275122feaa7`；Go 快照 `82570d905919c1c66bb025a3a5979f25390de498`。沿用已批准的 Java 线缆基线，不增加 Rust 方言猜测，不修改原 29 样例。

本轮补齐之前未覆盖的 6 类事务失败响应，再加入一个共名错误码对照。Go 原生 7 例均可编解码往返，Java/Rust 7 例均保留独立预期字段并输出 Java 字节；其中只有 4 例 Go/Java 字节相同。诊断程序检出已知差异而退出 0，表示诊断有效，不能作为剩余 3 例互操作验收。

| 样例 | 类型 | Java | 固定 Go | 当前结果 |
|---|---:|---|---|---|
| BranchRollbackResponseFailed | 6 | 失败码 4，分支状态 9 | 同 Java | 字节与字段相同 |
| GlobalRollbackResponseFailed | 10 | 失败码 3，全局状态 5 | 同 Java | 字节与字段相同 |
| BranchReportResponseFailed | 14 | 错误消息长度 2 字节 | 长度 1 字节 | **开放差异 GO-V1-FAIL-001** |
| GlobalStatusResponseFailed | 16 | 失败码 3，全局状态 0 | 同 Java | 字节与字段相同 |
| GlobalReportResponseFailed | 18 | 失败码 3，全局状态 4 | 同 Java | 字节与字段相同 |
| GlobalLockQueryResponseFailed | 22 | 错误消息长度 2 字节，lockable=false | 长度 1 字节 | **开放差异 GO-V1-FAIL-002** |
| BranchRegisterLockKeyConflictFailFast | 12 | 同名错误码 20 | 同名错误码 19 | **开放差异 GO-V1-FAIL-003** |

## 运行结果及影响

GO-V1-FAIL-001/002：固定 Java 对这两个 Go 样例抛出 `BufferUnderflowException`；Rust 面向 Java 的严格解码器也拒绝它们。Go 自己能往返这些字节，不能证明其能读取 Java 同类失败响应。

GO-V1-FAIL-003：Go 的 19 是 `LockKeyConflictFailFast`；Java 的 19 是 `NotRaftLeader`，20 才是 `LockKeyConflictFailFast`。Java/Rust 能解析 Go 的 19，但其语义会变为 NotRaftLeader。因此“可解码”并不证明同义。Rust 不能自动把合法 Java 19 改解释为 Go fail-fast，否则会破坏 Java 基线。

失败响应的状态与 `lockable` 不能覆盖 `ResultCode.Failed`。新增用例明确保留这些字段与错误消息；例如 Failed+lockable=false 不等于一次成功的锁冲突检查，Failed+RollbackRetrying 不等于已成功回滚。

来源：Go `pkg/protocol/codec/branch_statue_report_response_codec.go`、`global_lock_query_resp_codec.go`、`pkg/util/errors/code.go`；Java `AbstractResultMessageCodec.java` 和 `TransactionExceptionCode.java`。固定源码及原生字节实测见[本轮证据](evidence/local-failure-variants-20261112/README.md)。没有修改任何上游编解码器。

## 仍需完成的工作

### 真实 handler 响应的补充状态证据

后续135项 Java 运行捕获了78个可编码响应，Rust逐例字段与字节通过。固定 Go 对同一批输入得到62个覆盖字段／字节兼容、6个错误长度差异、1个错误码差异，以及9个**不透明状态**用例。差异用例次数与问题种类不同：6个长度用例仍归于上面的两个问题，1个错误码用例仍归于同一 fail-fast 问题。

`GO-V1-STATE-001`：这9个用例的 Java 全局状态为19（StopCommitOrCommitRetry），固定 Go 能保持数字19和原始字节，但其 `pkg/protocol/message/constant.go` 只命名了 GlobalStatus0..15。这里没有发现新的字节丢失；欠缺的是完整状态语义／业务行为证据。不能从数字往返直接推导 Go 已理解该状态，也不能反向声称所有 Go 调用者一定失败。该状态覆盖项继续开放，运行与源码哈希见[补充证据](evidence/local-handler-runtime-20261112/README.md)。

这些发现并未缩减完整计划的 Go 兼容目标。后续需要明确可验证的修复路径，例如评审上游兼容修复或显式版本适配，并分别验证修复前后语义、长错误文本及其他分叉错误码。未核实的后续枚举不能由这个 19/20 示例推导。

在完成相关评审与运行证据之前，上述三项保持开放；不把 Go 参考实现复制进正式协议，不通过改写既有 golden 字节掩盖差异，也不宣称所有失败变体已互操作。当前仅完成覆盖扩展及差异定位，不声称上游问题已经修复。

## 控制信封补充：GO-V1-MERGE-001

另一次固定源码实测增加九个Java控制帧：原始Go Getty对三个心跳逐字段/逐字节一致；六个合并样例在Java/Rust可往返，但Go的59/60模型没有注册原生codec。实际读取消费完整帧后body为nil，原registry无法生成有效合并payload。该缺口与上面三个失败响应问题分开编号，不能计为互操作通过，也没有通过注册替代codec掩盖。

Java163选定测试、Rust114探针测试、Python5采集/来源检查及静态检查通过，只证明所述诊断与样例。完整Go兼容目标仍开放，原29个常规样例及六个原有帧未改写。[字段/线缆映射](protocol-v1-control-envelopes.md)；[来源和运行证据](evidence/local-control-envelopes/README.md)。

## GO-V1-FAIL-001/002 的显式修复候选

另建的Go源码副本现有可复现长度补丁：type14/22改为16位读写及相应文本上限。原版在同一24个Java边界样例中21个失败响应失败、三个成功控制通过；补丁副本24项全部通过，受影响codec/bytes两包80个顶层测试、原29样例/六帧回归也通过。Java187、Rust115和Python6通过；来源、执行测试及精确用例身份均有校验。[补丁及适用范围](../../compatibility/seata-go/README.md)，[执行证据](evidence/local-go-java-length-patch/README.md)。

这是**有界候选缓解，固定原版问题仍开放**。没有覆盖原版源码、改提交或重写原样例；没有上游合入/发布证据。有效UTF-8字节长度0..32767和已验证ASCII截断之外，超大Unicode/畸形解码仍需后续处理。003错误码、STATE-001和MERGE-001并未由长度修复解决。

## GO-V1-FAIL-003 的显式响应适配候选

新增0002候选使用显式JavaV1ResponseProfile入口，保留Go0..34原枚举和默认registry；Java专有语义使用域值256..260。Java19不会再被该入口误解为Go fail-fast，Java20正确映射已有Go fail-fast，21–24逐项转换；Go专有错误和未知Java码返回错误，不能默认Unknown或byte截断。

固定Java实际250例（25异常码×10响应）验证：原版152匹配/98差异，仅0001长度修复190匹配/60差异，显式0001+0002入口250字段/字节及250指针/调用方保持检查全部通过。原来七个Java失败样例在该入口全部字段/字节通过，含001/002/003原始问题样例。Java437、Rust116、Go三包104顶层测试和新Python6通过。[补丁/API与限制](../../tools/go-java-v1-exception-profile/README.md)，[执行来源](evidence/local-go-java-exception-profile/README.md)。

原版默认API依然保留原差异；这不是上游发布/合入，也未接入Getty网络传输或自动选择peer版本。三个失败差异现有可审查候选缓解，完整客户端互操作仍开放。MERGE-001、STATE-001、超大Unicode/完整业务状态及生产Rust交付不因响应适配关闭。

## GO-V1-STATE-001 的完整枚举与辅助分类增量

2026-11-13 [独立运行证据](evidence/local-global-status-contract/README.md)补齐全部 Java 21 状态、九类辅助方法 189 观察与 SessionStatusValidator 441 对（108 拒绝）。四种全局响应的 168 个成功/失败样例在原 Go 与既有显式候选均保持字段/字节；Java 235 个未知状态拒绝，原 Go 对 940 个未知响应变体保留数字，显式入口全部拒绝。

原 Go 未命名范围现明确为 16–20 五项，而不是仅已有 handler 样例中的 19。18 Deleting 的 two-phase-success 与 end-global 分类不同，15 Finished 则为 heuristic/end；数字往返和校验器未拒绝不能替代业务转换可达性、恢复或调度验证。22 项新门禁检查、固定来源与实际产物复核通过；本项未修改生产实现，GO-V1-STATE-001 继续 OPEN。

## GO-V1-MERGE-001 载荷阶段候选

显式0003载荷入口现已覆盖25普通类型与59/60，38个新Java组合/边界样例及原六个合并样例全部通过；250异常语义在合并结果中保持，29普通载荷也回归。新有符号超时与缺省版本问题先被审查/失败测试暴露，再以显式转换及WireVersion/Java getter元数据修正。Java475/Rust117/Go110和新Python7通过。[证据](evidence/local-go-java-merge-profile/README.md)。

原版registry与Getty默认读写路径保持原行为；下一阶段仍须接入并验证实际帧读写及错误传播。因此这是有界载荷候选，不关闭原版/完整客户端合并互操作，也不改变Phase0或正式组件的验收。
