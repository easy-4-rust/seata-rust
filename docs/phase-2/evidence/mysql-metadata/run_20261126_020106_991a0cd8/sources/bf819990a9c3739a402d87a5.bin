use crate::ProtocolError;

/// RequestType协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.protocol.ProtocolConstants 的 MSGTYPE 常量
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestType {
    /// 同步请求。
    RequestSync,
    /// 响应。
    Response,
    /// 不要求响应的单向请求。
    RequestOneway,
    /// 空载荷心跳请求。
    HeartbeatRequest,
    /// 空载荷心跳响应。
    HeartbeatResponse,
}

impl RequestType {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::RequestSync),
            1 => Ok(Self::Response),
            2 => Ok(Self::RequestOneway),
            3 => Ok(Self::HeartbeatRequest),
            4 => Ok(Self::HeartbeatResponse),
            _ => Err(ProtocolError::UnknownCode {
                kind: "RequestType",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u8 {
        match self {
            Self::RequestSync => 0,
            Self::Response => 1,
            Self::RequestOneway => 2,
            Self::HeartbeatRequest => 3,
            Self::HeartbeatResponse => 4,
        }
    }
}
