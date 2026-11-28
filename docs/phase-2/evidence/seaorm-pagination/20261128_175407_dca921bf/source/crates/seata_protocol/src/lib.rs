//! 与固定 Java Seata 基线对应的协议域模型及严格线缆编解码。
//! 协议层不负责 TCP、事务重试或资源执行。

/// 严格且有界的原生V1字段读写接口。
pub mod codec;

mod compressor;
mod exception;
mod model;
/// 原生V1身份与消息类型模型。
pub mod protocol;
mod serializer;
/// 原生V1事务字段模型，按固定Java消息逐个组织。
pub mod transaction;
/// 原生V1完整帧、有序可空头与显式资源预算。
pub mod v1;

pub use compressor::CompressorType;
pub use exception::{ProtocolError, TransactionExceptionCode};
pub use model::{BranchStatus, BranchType, GlobalStatus};
pub use protocol::{Message, MessageType, RequestType, ResultCode};
pub use serializer::{PayloadCodec, PayloadLimits, SerializerType};
