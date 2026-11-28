use crate::ProtocolError;

/// SerializerType协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.serializer.SerializerType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SerializerType {
    /// Seata原生序列化，首期实现。
    Seata,
    /// Protobuf，需要对应扩展。
    Protobuf,
    /// Kryo扩展。
    Kryo,
    /// 历史FST编号；当前Java已移除扩展，首期不支持。
    Fst,
    /// Hessian扩展。
    Hessian,
    /// Jackson扩展。
    Jackson,
    /// Fastjson2扩展。
    Fastjson2,
    /// gRPC扩展，不是原生V1的替代。
    Grpc,
    /// Fury历史编号。
    Fury,
    /// Fory扩展。
    Fory,
}

impl SerializerType {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u8) -> Result<Self, ProtocolError> {
        match code {
            1 => Ok(Self::Seata),
            2 => Ok(Self::Protobuf),
            4 => Ok(Self::Kryo),
            8 => Ok(Self::Fst),
            22 => Ok(Self::Hessian),
            50 => Ok(Self::Jackson),
            100 => Ok(Self::Fastjson2),
            40 => Ok(Self::Grpc),
            86 => Ok(Self::Fury),
            87 => Ok(Self::Fory),
            _ => Err(ProtocolError::UnknownCode {
                kind: "SerializerType",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u8 {
        match self {
            Self::Seata => 1,
            Self::Protobuf => 2,
            Self::Kryo => 4,
            Self::Fst => 8,
            Self::Hessian => 22,
            Self::Jackson => 50,
            Self::Fastjson2 => 100,
            Self::Grpc => 40,
            Self::Fury => 86,
            Self::Fory => 87,
        }
    }
}
