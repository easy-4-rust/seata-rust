#![allow(clippy::unwrap_used)]
use seata_protocol::protocol::RpcMessage;
use seata_protocol::transaction::{
    AbstractTransactionResponse, GlobalBeginRequest, GlobalBeginResponse, GlobalStatusResponse,
};
use seata_protocol::v1::{FrameLimits, ProtocolDecoderV1, ProtocolEncoderV1};
use seata_protocol::{
    CompressorType, GlobalStatus, Message, PayloadLimits, RequestType, ResultCode, SerializerType,
    TransactionExceptionCode,
};
use seata_transport::ConnectionConfig;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub fn limits() -> FrameLimits {
    FrameLimits::new(
        64 * 1024,
        32767,
        128,
        PayloadLimits::new(64 * 1024, 512, 1024, 16).unwrap(),
    )
    .unwrap()
}
pub fn config(maximum: usize) -> ConnectionConfig {
    ConnectionConfig::new(
        limits(),
        maximum,
        Duration::from_secs(2),
        Duration::from_secs(2),
    )
    .unwrap()
}
pub fn begin(name: &str) -> Message {
    Message::GlobalBeginRequest(GlobalBeginRequest {
        timeout: 30000,
        transaction_name: Some(name.into()),
    })
}
pub fn success() -> AbstractTransactionResponse {
    AbstractTransactionResponse {
        result_code: ResultCode::Success,
        msg: None,
        transaction_exception_code: TransactionExceptionCode::Unknown,
    }
}
pub fn begin_result(xid: &str) -> Message {
    Message::GlobalBeginResponse(GlobalBeginResponse {
        response: success(),
        xid: Some(xid.into()),
        extra_data: None,
    })
}
pub fn status_result() -> Message {
    Message::GlobalStatusResponse(GlobalStatusResponse {
        response: success(),
        global_status: GlobalStatus::Committed,
    })
}
pub fn reply(id: i32, body: Message) -> RpcMessage {
    RpcMessage {
        id,
        message_type: RequestType::Response,
        codec: SerializerType::Seata,
        compressor: CompressorType::None,
        head_map: vec![],
        body: Some(body),
    }
}
pub fn heartbeat(id: i32, kind: RequestType) -> RpcMessage {
    RpcMessage {
        id,
        message_type: kind,
        codec: SerializerType::Seata,
        compressor: CompressorType::None,
        head_map: vec![],
        body: None,
    }
}
pub fn xid(value: RpcMessage) -> String {
    match value.body.unwrap() {
        Message::GlobalBeginResponse(v) => v.xid.unwrap(),
        _ => panic!("wrong response family"),
    }
}
pub async fn read(stream: &mut TcpStream) -> RpcMessage {
    // 固定头+declared body的朴素读取与客户端增量解码器独立，不共享其分片逻辑。
    let mut fixed = [0u8; 16];
    tokio::time::timeout(Duration::from_secs(3), stream.read_exact(&mut fixed))
        .await
        .unwrap()
        .unwrap();
    let length = u32::from_be_bytes([fixed[3], fixed[4], fixed[5], fixed[6]]) as usize;
    assert!((16..=65536).contains(&length));
    let mut bytes = fixed.to_vec();
    bytes.resize(length, 0);
    tokio::time::timeout(Duration::from_secs(3), stream.read_exact(&mut bytes[16..]))
        .await
        .unwrap()
        .unwrap();
    ProtocolDecoderV1::new(limits()).decode(&bytes).unwrap()
}
pub async fn write(stream: &mut TcpStream, frame: RpcMessage) {
    let bytes = ProtocolEncoderV1::new(limits()).encode(&frame).unwrap();
    // 真正的TCP小段写入，客户端不能依赖一次read得到一帧。
    for part in [&bytes[..1], &bytes[1..7], &bytes[7..]] {
        tokio::time::timeout(Duration::from_secs(3), stream.write_all(part))
            .await
            .unwrap()
            .unwrap();
        tokio::task::yield_now().await;
    }
}
pub async fn eof(stream: &mut TcpStream) {
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), stream.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}
