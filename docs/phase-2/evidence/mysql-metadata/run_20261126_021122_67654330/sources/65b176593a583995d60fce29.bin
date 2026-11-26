use crate::ConnectionStats;

/// 池生命周期内各连接代际的累计计数；不含当前在途数、业务标识或原始报文。
/// 对应 Java: NettyClientChannelManager 的池级监控职责。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PoolCounters {
    /// 首次轮询普通请求写入的总次数。
    pub write_attempted: u64,
    /// 首次轮询普通回包写入的总次数。
    pub response_write_attempted: u64,
    /// 本地完整写入普通回包的总次数，不表示对端确认。
    pub responses_written: u64,
    /// 移交有效响应的总次数，包括失败 ResultCode。
    pub completed: u64,
    /// 写前期限届满总次数。
    pub timed_out_before_send: u64,
    /// 写后期限届满总次数，结果可能已生效。
    pub timed_out_after_send: u64,
    /// 写前取消总次数。
    pub cancelled_before_send: u64,
    /// 写后取消总次数。
    pub cancelled_after_send: u64,
    /// 无匹配等待项的响应总次数。
    pub unmatched_responses: u64,
    /// 收到空心跳的总次数。
    pub heartbeats_received: u64,
    /// 完整写入定时 PING 的总次数。
    pub heartbeat_requests_sent: u64,
    /// 完整写入 PONG 的总次数。
    pub heartbeat_responses_sent: u64,
    /// 因读空闲结束的连接总次数。
    pub read_idle_expirations: u64,
    /// 成功交付入站队列的请求总次数。
    pub inbound_requests: u64,
}

impl PoolCounters {
    pub(crate) fn from_connection(stats: &ConnectionStats) -> Self {
        Self {
            write_attempted: stats.write_attempted,
            response_write_attempted: stats.response_write_attempted,
            responses_written: stats.responses_written,
            completed: stats.completed,
            timed_out_before_send: stats.timed_out_before_send,
            timed_out_after_send: stats.timed_out_after_send,
            cancelled_before_send: stats.cancelled_before_send,
            cancelled_after_send: stats.cancelled_after_send,
            unmatched_responses: stats.unmatched_responses,
            heartbeats_received: stats.heartbeats_received,
            heartbeat_requests_sent: stats.heartbeat_requests_sent,
            heartbeat_responses_sent: stats.heartbeat_responses_sent,
            read_idle_expirations: stats.read_idle_expirations,
            inbound_requests: stats.inbound_requests,
        }
    }

    pub(crate) fn accumulate(&mut self, next: Self) {
        self.write_attempted = self.write_attempted.saturating_add(next.write_attempted);
        self.response_write_attempted = self
            .response_write_attempted
            .saturating_add(next.response_write_attempted);
        self.responses_written = self
            .responses_written
            .saturating_add(next.responses_written);
        self.completed = self.completed.saturating_add(next.completed);
        self.timed_out_before_send = self
            .timed_out_before_send
            .saturating_add(next.timed_out_before_send);
        self.timed_out_after_send = self
            .timed_out_after_send
            .saturating_add(next.timed_out_after_send);
        self.cancelled_before_send = self
            .cancelled_before_send
            .saturating_add(next.cancelled_before_send);
        self.cancelled_after_send = self
            .cancelled_after_send
            .saturating_add(next.cancelled_after_send);
        self.unmatched_responses = self
            .unmatched_responses
            .saturating_add(next.unmatched_responses);
        self.heartbeats_received = self
            .heartbeats_received
            .saturating_add(next.heartbeats_received);
        self.heartbeat_requests_sent = self
            .heartbeat_requests_sent
            .saturating_add(next.heartbeat_requests_sent);
        self.heartbeat_responses_sent = self
            .heartbeat_responses_sent
            .saturating_add(next.heartbeat_responses_sent);
        self.read_idle_expirations = self
            .read_idle_expirations
            .saturating_add(next.read_idle_expirations);
        self.inbound_requests = self.inbound_requests.saturating_add(next.inbound_requests);
    }
}
