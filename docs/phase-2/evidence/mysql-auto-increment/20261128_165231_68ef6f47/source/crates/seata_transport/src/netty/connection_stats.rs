use crate::{ConnectionGeneration, TransportError};

/// 一致的单连接计数快照，不含认证字段、XID、资源URL或原始帧。
/// 对应 Java: AbstractNettyRemoting#futures 与通道观测职责；新增Rust完成所有权计数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionStats {
    /// 连接代际，和请求编号独立。
    pub generation: ConnectionGeneration,
    /// 当前等待响应数，不包含已移交给调用方的结果。
    pub pending: usize,
    /// 已进入首次写入轮询的请求数。
    pub write_attempted: u64,
    /// 开放通道中已轮询首次网络写入的普通回包数，与请求future无关。
    pub response_write_attempted: u64,
    /// 开放通道中已完整本地写完的普通回包数，不代表TC确认。
    pub responses_written: u64,
    /// 已移交的有效响应数，失败ResultCode也属于权威响应。
    pub completed: u64,
    /// 写入前期限届满的调用数。
    pub timed_out_before_send: u64,
    /// 写入后期限届满、结果未知的调用数。
    pub timed_out_after_send: u64,
    /// 写入前取消的等待调用数。
    pub cancelled_before_send: u64,
    /// 写入后取消的等待调用数，不推断TC取消事务。
    pub cancelled_after_send: u64,
    /// 无等待项的晚到/重复/未知ID响应数，不存储无界历史字典。
    pub unmatched_responses: u64,
    /// 收到的空心跳请求/响应数。
    pub heartbeats_received: u64,
    /// 已完整写入的定时PING数，不计普通请求或仅入队的控制帧。
    pub heartbeat_requests_sent: u64,
    /// 已完整写入的PONG回复数。
    pub heartbeat_responses_sent: u64,
    /// 以读空闲为首次原因终止的次数（单连接最多一次）。
    pub read_idle_expirations: u64,
    /// 交付给该连接入站队列的请求数。
    pub inbound_requests: u64,
    /// 连接是否已永久终止。
    pub closed: bool,
    /// 首次终止原因，仅含静态协议/OS元数据。
    pub last_failure: Option<TransportError>,
}
impl ConnectionStats {
    pub(crate) fn new(generation: ConnectionGeneration) -> Self {
        Self {
            generation,
            pending: 0,
            write_attempted: 0,
            response_write_attempted: 0,
            responses_written: 0,
            completed: 0,
            timed_out_before_send: 0,
            timed_out_after_send: 0,
            cancelled_before_send: 0,
            cancelled_after_send: 0,
            unmatched_responses: 0,
            heartbeats_received: 0,
            heartbeat_requests_sent: 0,
            heartbeat_responses_sent: 0,
            read_idle_expirations: 0,
            inbound_requests: 0,
            closed: false,
            last_failure: None,
        }
    }
}
