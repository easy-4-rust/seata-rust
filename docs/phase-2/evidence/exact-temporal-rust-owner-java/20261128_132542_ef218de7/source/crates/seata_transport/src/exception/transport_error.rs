use seata_protocol::ProtocolError;
use thiserror::Error;

/// 连接基础设施原因；请求结果由CallError另外区分写入前失败与未知结果。
/// 对应 Java: org.apache.seata.core.exception.DecodeException 及Netty连接生命周期错误；Rust显式分类。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TransportError {
    /// 保留OS错误种类，不包含原始请求/认证数据。
    #[error("网络I/O失败：{kind:?}")]
    Io {
        /// 标准库错误种类。
        kind: std::io::ErrorKind,
    },
    /// TLS 证书、对端身份或加密握手失败；不包含证书或底层错误详情。
    #[error("TLS 身份验证或握手失败")]
    TlsHandshakeFailed,
    /// TCP建立尚未完成，不曾写入请求帧。
    #[error("TCP连接超时")]
    ConnectTimeout,
    /// 已开始的帧写入超过配置上限，必须关闭连接以免复用半帧流。
    #[error("帧写入超时")]
    WriteTimeout,
    /// 请求的传输响应期限届满，是否未知由写入标记决定。
    #[error("等待响应超时")]
    ResponseTimeout,
    /// 完整有效入站帧的绝对空闲期限届满；片段字节活动不能刷新此期限。
    #[error("TCP会话读空闲超时")]
    ReadIdleTimeout,
    /// 获取raw池通道的绝对期限届满；尚未分发业务请求。
    #[error("等待连接池通道超时")]
    PoolAcquireTimeout,
    /// 池已永久关闭，不能重新获取或重建通道。
    #[error("连接池已经关闭")]
    PoolClosed,
    /// 有效原生identified=false，索引None表示身份，Some表示所需资源位置。
    #[error("会话登记被拒绝，资源索引 {resource_index:?}")]
    RegistrationRejected {
        /// 仅有界索引，不输出资源ID/身份/extraData。
        resource_index: Option<usize>,
    },
    /// 初始化未能完成，不能伪造为TC明确拒绝或ready。
    #[error("会话登记尚未完成")]
    RegistrationIncomplete,
    /// 会话或其后台任务已经终止。
    #[error("TCP会话已经终止")]
    ConnectionClosed,
    /// 有界等待表或发送队列已满，请求未写入。
    #[error("连接等待表或发送队列已满")]
    Overloaded,
    /// 已知关联ID的信封/类型/合并形状不符合该请求。
    #[error("响应不符合请求：{reason}")]
    UnexpectedResponse {
        /// 不包含字段内容的静态原因。
        reason: &'static str,
    },
    /// 任务或同步状态异常，连接必须终止。
    #[error("连接内部状态异常：{reason}")]
    InternalState {
        /// 静态诊断原因。
        reason: &'static str,
    },
    /// 完整协议错误被保留；该错误会终止流解码器，不能吞错后在同一流恢复。
    #[error("协议流无效：{0}")]
    Protocol(#[from] ProtocolError),
    /// EOF或错误后，该连接的流解码器已经终止。
    #[error("该连接的流解码器已经终止")]
    DecoderClosed,
    /// 连接在不完整帧中结束，不能将残片接入下一连接。
    #[error("连接在不完整帧中结束：已缓冲 {buffered} 字节，预期 {expected_frame_length:?}")]
    UnexpectedEof {
        /// 实际已缓冲的字节数。
        buffered: usize,
        /// 固定头完整有效时的整帧长度，否则None。
        expected_frame_length: Option<usize>,
    },
    /// 即使预算合法，底层分配仍可能失败，保持错误路径而非返回半帧。
    #[error("无法分配帧缓冲区，目标字节数 {requested}")]
    BufferAllocationFailed {
        /// 已验证在帧预算内的目标长度。
        requested: usize,
    },
    /// 配置无法满足编号或连接状态约束。
    #[error("连接配置无效：{reason}")]
    InvalidConfiguration {
        /// 不包含敏感参数内容的静态原因。
        reason: &'static str,
    },
    /// 单连接的编号额度已用尽；必须更换连接，不能在本连接回收旧编号。
    #[error("连接请求编号额度 {maximum} 已用完，已发出 {issued} 个编号；需要更换连接")]
    RequestIdRotationRequired {
        /// 已发出的编号数，不因完成/超时而递减。
        issued: u64,
        /// 此连接配置的编号额度。
        maximum: u64,
    },
}
