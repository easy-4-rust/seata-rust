use crate::ProtocolError;

/// ResultCode协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.protocol.ResultCode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultCode {
    /// 调用明确失败；其他状态字段不能覆盖此结果。
    Failed,
    /// 调用成功；具体完成程度仍由业务状态说明。
    Success,
}

impl ResultCode {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::Failed),
            1 => Ok(Self::Success),
            _ => Err(ProtocolError::UnknownCode {
                kind: "ResultCode",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u8 {
        match self {
            Self::Failed => 0,
            Self::Success => 1,
        }
    }
}
