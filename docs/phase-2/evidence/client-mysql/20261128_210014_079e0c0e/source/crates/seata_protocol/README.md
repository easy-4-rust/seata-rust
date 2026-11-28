# seata-protocol

正式 Seata-Rust workspace 的协议组件，已实现九组协议域枚举、显式 Java V1 双向码映射、类型化协议错误、有界大端字段读写，以及25类普通消息和59/60聚合的字段模型与载荷编解码。

完整V1帧/HeadMap/心跳已实现；该crate不承担TCP连接、事务重试或数据库执行，当前不能作为完整客户端使用，发布开关暂为关闭。

```rust
use seata_protocol::{codec::WireWriter, GlobalStatus, ProtocolError};

fn fields() -> Result<Vec<u8>, ProtocolError> {
    let status = GlobalStatus::from_java_v1(19)?;
    assert_eq!(status, GlobalStatus::StopCommitOrCommitRetry);
    let mut writer = WireWriter::new(4096);
    writer.string16(Some("订单"), "transaction_name")?;
    writer.u8(status.to_java_v1())?;
    Ok(writer.into_bytes())
}
```

这只是字段组合示例。普通string16的0读取为None，消息模型按各Java字段分别映射为None或空串；HeadMap另用nullable接口表示-1=None、0=Some空串。未知码明确拒绝；已知但未实现的消息返回UnsupportedMessage。`in_v1_payload_allowlist`表达批准范围，`PayloadCodec::supports`表达27类实现能力；它不保证具体对象符合实例预算。

```rust
use seata_protocol::{transaction::GlobalBeginRequest, Message, PayloadCodec, ProtocolError};

fn begin_payload() -> Result<Vec<u8>, ProtocolError> {
    let codec = PayloadCodec::new(4096)?; // 调用方显式指定字节预算。
    let request = Message::GlobalBeginRequest(GlobalBeginRequest {
        timeout: 30_000,
        transaction_name: Some("订单".into()),
    });
    let payload = codec.encode(&request)?; // 含两字节消息类型，不含TCP帧头。
    assert_eq!(codec.decode(&payload)?, request);
    Ok(payload)
}
```

事务响应保留result_code、异常码与各消息自己的状态/lockable，后续状态不能覆盖Failed。GlobalReport的255表示None，与状态0区分。注册响应保留原始wire_version，java_version需调用方传入当前Java构建默认值。Java解码器遇零长会提前返回的必需字段，严格入口拒绝不完整对象；不默默丢弃成功响应上的错误文本。资源标识/锁键按原文本保留，本轮没有恢复已延期的共享锁工作。

普通消息继续使用`PayloadCodec::new(max_bytes)`，聚合默认禁用；聚合需使用`PayloadCodec::with_limits(PayloadLimits::new(max_bytes, max_children, max_nodes, max_depth)?)`显式配置四种预算。节点数含根和嵌套容器，层数只计聚合容器，0禁用聚合，最大64限制递归栈；单批最多32767是signed-short表示范围。字节数包括所有类型及长度前缀。业务可选择更小预算，不继承旧探针的固定8MiB/1024/2048/8。

59的msg_ids要么全部省略，要么与msgs等长；60只接受12类普通结果子消息，其未设置数组用None表示并拒绝编码，Some空列表合法。解码用封闭的子游标，逐条分配且整棵树共享节点计数；编码在同一缓冲区回填长度，避免嵌套复制。原Go默认registry缺少59/60的事实保持OPEN，正式Rust支持不代表原Go已兼容。

所有域/方法有中文文档及Java来源。每个对象独立文件，crate入口只声明模块和明确重导出。读写器要求调用方显式提供长度预算；超长写入不截断UTF-8或留下部分字段，Debug不展示原始载荷内容。

```text
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
```

测试覆盖已增至74项：原29载荷独立语义预期、425条既有实际Java记录（250异常/168状态/7补充失败）、六控制聚合与38扩展聚合、78处理器上下文侧车、六原始帧/九控制帧和37条本轮新Java实际帧，以及边界/预算/畸形输入。37新帧确实运行原Java编码/解码器，覆盖25普通类型及null/empty/ordered头、32767字节头边界、signed ID/oneway；不是JUnit或现场TC测试。其余记录来自历史实际Java运行，不重复计作本轮Java执行。

`v1::ProtocolEncoderV1`/`ProtocolDecoderV1`一次处理一个完整帧。`FrameLimits::new(max_frame_bytes, max_head_bytes, max_header_entries, payload_limits)`要求16<=头预算<=帧预算，头最多32767、帧最多Java int范围，全部由调用方显式提供。头预算包含固定16字节；payload字节还受实际剩余帧空间限制。`protocol::RpcMessage`保留signed id、outer RequestType、codec/compressor、有序头和可选body；None只用于空心跳，普通帧必须Some，不加入120前缀。

头的空key合法，null key/重复key/-2等非规范负长明确拒绝；这些是现行严格profile要求，不声称原Java也如此严格。解码完整校验magic/version/fullLength/headLength/body及尾随字节。所有字符串/头内容的Debug均不展示原文，头key在哈希前先通过字节预算。TCP分片/连续帧拆分由后续连接组件负责。

连接层现复用新增的`ProtocolDecoderV1::frame_length(prefix)`：只检查已到齐的固定头字段，不完整且可能有效返回None，完整有效头返回整帧长度；不解析HeadMap/正文，也不因后续连续帧字节而拒绝。完整decode与该方法共用固定头规则，仍只接受一条完整帧。增量组帧由[seata-transport](../seata_transport/README.md)负责。

只读核验工具为tools/protocol-fixtures/verify_formal_{payload,merge,frame}_oracles.py。[批准初始peer-profile Gate](../../docs/phase-1/evidence/protocol-peer-profile/README.md)现已通过：628正向字段/字节及2050畸形输入由正式Rust和显式Go适配器真实执行比较，独立Java语义作预期；不是仅凭单元测试关闭门禁。原Go默认兼容差异、WP00-R和完整客户端/发布条件仍开放。

正式依赖锁在根Cargo.lock。测试中的unwrap只用于让意外错误直接失败，生产API不使用unwrap/expect。当前仍非完整WP-01验收或产品发布。
