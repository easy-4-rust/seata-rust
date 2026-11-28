/// 日志适配器本地配置错误，不包含输出路径、身份或凭据。
/// 对应 Java: 无直接对象；WP07 Rust 日志配置校验。
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum StructuredLogError {
    /// 容量、单条大小或二者乘积不在允许范围。
    #[error("structured log capacity/record byte budget is invalid")]
    InvalidConfiguration,
}
