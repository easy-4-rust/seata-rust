use crate::ProtocolError;

/// BranchType协议域模型，通过显式方法转换Java V1码，不依赖Rust声明顺序。
/// 对应 Java: org.apache.seata.core.model.BranchType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchType {
    /// AT模式。
    At,
    /// TCC模式；域可表示不代表首期RM已实现该引擎。
    Tcc,
    /// Saga模式。
    Saga,
    /// XA模式。
    Xa,
    /// 注解Saga模式。
    SagaAnnotation,
}

impl BranchType {
    /// 将参数`code`映射到Java V1域值；未知码返回带原值的协议错误。
    pub fn from_java_v1(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::At),
            1 => Ok(Self::Tcc),
            2 => Ok(Self::Saga),
            3 => Ok(Self::Xa),
            4 => Ok(Self::SagaAnnotation),
            _ => Err(ProtocolError::UnknownCode {
                kind: "BranchType",
                code: u64::from(code),
            }),
        }
    }

    /// 返回此域值的Java V1线缆码；不使用枚举ordinal或整数强制转换。
    pub const fn to_java_v1(self) -> u8 {
        match self {
            Self::At => 0,
            Self::Tcc => 1,
            Self::Saga => 2,
            Self::Xa => 3,
            Self::SagaAnnotation => 4,
        }
    }
}
