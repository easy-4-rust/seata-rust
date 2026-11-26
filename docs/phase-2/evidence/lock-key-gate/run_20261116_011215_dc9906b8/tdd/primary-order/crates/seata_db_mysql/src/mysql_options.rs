use crate::DbError;
use seata_datasource::TableMeta;
use sqlx::mysql::{MySqlConnectOptions, MySqlSslMode};
use sqlx::ConnectOptions;
use std::{fmt, net::IpAddr, time::Duration};

/// 有限连接预算与显式TLS配置；Debug不显示认证字符串。
/// 对应 Java: DataSourceProxy的原始数据源配置；Rust独立配置对象。
#[derive(Clone)]
pub struct MySqlOptions {
    host: String,
    port: u16,
    username: String,
    password: String,
    pub(crate) database: String,
    insecure_loopback: bool,
    pub(crate) maximum_connections: u32,
    pub(crate) acquire_timeout: Duration,
    pub(crate) operation_timeout: Duration,
}
impl MySqlOptions {
    /// 参数为主机、端口、原认证字段和schema；返回有限预算且默认校验TLS身份的配置。
    pub fn new(
        host: String,
        port: u16,
        username: String,
        password: String,
        database: String,
    ) -> Result<Self, DbError> {
        TableMeta::validate_identifier(&database)?;
        if host.is_empty()
            || host.len() > 253
            || host.chars().any(|c| c.is_control() || c.is_whitespace())
            || host.contains(['/', '@', '\\'])
            || port == 0
            || username.is_empty()
            || username.len() > 128
            || username.chars().any(char::is_control)
        {
            return Err(DbError::Invalid {
                reason: "host/port/username",
            });
        }
        Ok(Self {
            host,
            port,
            username,
            password,
            database,
            insecure_loopback: false,
            maximum_connections: 4,
            acquire_timeout: Duration::from_secs(3),
            operation_timeout: Duration::from_secs(3),
        })
    }
    /// 无参数；仅允许字面loopback IP显式关闭测试TLS，非loopback/DNS名称返回错误。
    pub fn with_insecure_loopback_tls(mut self) -> Result<Self, DbError> {
        if !self.host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()) {
            return Err(DbError::Invalid {
                reason: "insecure TLS requires literal loopback",
            });
        }
        self.insecure_loopback = true;
        Ok(self)
    }
    /// 参数为最大连接数及获取等待；返回校验后的配置，禁止零值和无限预算。
    pub fn pool_limits(mut self, maximum: u32, acquire_timeout: Duration) -> Result<Self, DbError> {
        if !(1..=32).contains(&maximum)
            || acquire_timeout.is_zero()
            || acquire_timeout > Duration::from_secs(60)
        {
            return Err(DbError::Invalid {
                reason: "pool limits",
            });
        }
        self.maximum_connections = maximum;
        self.acquire_timeout = acquire_timeout;
        Ok(self)
    }
    /// 参数为一次完整数据库操作的客户端预算；返回校验配置，覆盖该调用的所有子SQL。
    pub fn operation_timeout(mut self, timeout: Duration) -> Result<Self, DbError> {
        if timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(DbError::Invalid {
                reason: "operation timeout",
            });
        }
        self.operation_timeout = timeout;
        Ok(self)
    }
    /// 无参数；返回关闭参数日志的原SQLx连接配置，不在公开Debug暴露该对象。
    pub(crate) fn connect_options(&self) -> MySqlConnectOptions {
        MySqlConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.username)
            .password(&self.password)
            .database(&self.database)
            .charset("utf8mb4")
            .ssl_mode(if self.insecure_loopback {
                MySqlSslMode::Disabled
            } else {
                MySqlSslMode::VerifyIdentity
            })
            .log_statements(log::LevelFilter::Off)
            .log_slow_statements(log::LevelFilter::Off, Duration::from_secs(1))
    }
}
impl fmt::Debug for MySqlOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MySqlOptions")
            .field("host_bytes", &self.host.len())
            .field("port", &self.port)
            .field("username_bytes", &self.username.len())
            .field("database_bytes", &self.database.len())
            .field("insecure_loopback", &self.insecure_loopback)
            .field("maximum_connections", &self.maximum_connections)
            .finish_non_exhaustive()
    }
}
