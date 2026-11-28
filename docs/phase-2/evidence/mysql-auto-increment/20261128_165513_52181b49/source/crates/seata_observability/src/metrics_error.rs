/// 指标服务配置、绑定及所有者任务错误；不包含凭据或请求头。
/// 对应 Java: 监控端点启动与关闭异常职责。
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    /// 不安全或无效的导出配置。
    #[error("invalid metrics configuration: {reason}")]
    InvalidConfiguration {
        /// 不含用户输入的静态失败原因。
        reason: &'static str,
    },
    /// 本机监听失败。
    #[error("metrics listener bind failed: {kind:?}")]
    Bind {
        /// 监听失败的操作系统错误类别。
        kind: std::io::ErrorKind,
    },
    /// 已启动的监听器接收连接失败。
    #[error("metrics listener accept failed: {kind:?}")]
    Accept {
        /// 接收连接失败的操作系统错误类别。
        kind: std::io::ErrorKind,
    },
    /// 导出任务异常结束。
    #[error("metrics exporter task failed")]
    WorkerFailed,
}
