use thiserror::Error;

/// 严格协议校验的类型化错误，不将无效数据折叠为业务默认值。
/// 对应 Java: MessageSeataCodec / ProtocolDecoderV1 的参数、长度及缓冲区错误边界。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// 对象字段无法在该消息的V1布局中无损表达，或违反完整消息的必需字段约束。
    #[error("{field} 的值无效：{reason}")]
    InvalidValue {
        /// 无法表达或不完整的字段。
        field: &'static str,
        /// 校验失败的静态原因，不包含敏感字段内容。
        reason: &'static str,
    },
    /// 对应枚举不存在该码值；保留字段种类和原始码值。
    #[error("未知 {kind} 线缆码：{code}")]
    UnknownCode {
        /// 产生未知码的协议域类型。
        kind: &'static str,
        /// 未经截断的原始线缆码值。
        code: u64,
    },
    /// 当前已知消息没有在首期载荷allowlist中实现。
    #[error("尚未支持的消息类型：{code}")]
    UnsupportedMessage {
        /// 未经截断的原始线缆码值。
        code: u16,
    },
    /// 当前peer profile尚未实现该已知序列化器。
    #[error("尚未支持的序列化器：{code}")]
    UnsupportedSerializer {
        /// 未经截断的原始线缆码值。
        code: u8,
    },
    /// 当前peer profile尚未实现该已知压缩器。
    #[error("尚未支持的压缩器：{code}")]
    UnsupportedCompressor {
        /// 未经截断的原始线缆码值。
        code: u8,
    },
    /// 输入不足，读取前检查失败；不会返回部分对象。
    #[error("{field} 被截断，需要 {needed} 字节，仅剩 {remaining} 字节")]
    Truncated {
        /// 错误所属的字段名。
        field: &'static str,
        /// 本次读取要求的字节数。
        needed: usize,
        /// 输入中尚未消费的字节数。
        remaining: usize,
    },
    /// 线缆长度具有非法符号或超出表示范围。
    #[error("{field} 长度无效：{length}")]
    InvalidLength {
        /// 错误所属的字段名。
        field: &'static str,
        /// 原始有符号长度。
        length: i64,
    },
    /// 已解码完一个完整对象，但输入仍有未消费数据。
    #[error("完整对象之后仍有 {remaining} 字节")]
    TrailingBytes {
        /// 尚未消费的字节数。
        remaining: usize,
    },
    /// UTF-8数据非法，不用替代字符吞掉损坏输入。
    #[error("{field} 不是有效 UTF-8")]
    InvalidUtf8 {
        /// 错误所属的字段名。
        field: &'static str,
    },
    /// 超出调用方明确配置的资源预算。
    #[error("{limit} 超过预算 {maximum}，实际 {actual}")]
    LimitExceeded {
        /// 被超出的资源预算名称。
        limit: &'static str,
        /// 调用方配置的最大值。
        maximum: usize,
        /// 本次实际或溢出后饱和的需求量。
        actual: usize,
    },
    /// 帧字段之间不满足V1约束。
    #[error("V1 帧无效：{reason}")]
    InvalidFrame {
        /// 校验失败原因。
        reason: &'static str,
    },
    /// 布尔字段不使用0或1编码。
    #[error("{field} 布尔码无效：{value}")]
    InvalidBoolean {
        /// 错误所属的字段名。
        field: &'static str,
        /// 原始不规范布尔码。
        value: u16,
    },
    /// 预算等本地配置不能形成可执行的协议设置。
    #[error("协议配置无效：{reason}")]
    InvalidConfiguration {
        /// 校验失败原因。
        reason: &'static str,
    },
}
