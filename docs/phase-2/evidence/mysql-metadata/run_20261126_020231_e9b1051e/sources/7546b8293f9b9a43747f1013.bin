use crate::DbError;
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
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
    tls_ca_pem: Option<Vec<u8>>,
    tls_client_identity_pem: Option<(Vec<u8>, Vec<u8>)>,
    pub(crate) require_resource_binding: bool,
    pub(crate) require_tinyint_profile_binding: bool,
    pub(crate) java_tc_mysql_lock_limits: bool,
    pub(crate) java_tc_mysql_lock_collation: bool,
    pub(crate) maximum_connections: u32,
    pub(crate) acquire_timeout: Duration,
    pub(crate) operation_timeout: Duration,
}
impl MySqlOptions {
    /// 参数为可连接的主机、端口、原认证字段和schema；拒绝未指定IP及空白用户名，默认校验TLS身份。
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
            || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_unspecified())
            || port == 0
            || username.trim().is_empty()
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
            tls_ca_pem: None,
            tls_client_identity_pem: None,
            require_resource_binding: false,
            require_tinyint_profile_binding: false,
            java_tc_mysql_lock_limits: false,
            java_tc_mysql_lock_collation: false,
            maximum_connections: 4,
            acquire_timeout: Duration::from_secs(3),
            operation_timeout: Duration::from_secs(3),
        })
    }
    /// 无参数；仅允许字面loopback IP显式关闭测试TLS，非loopback/DNS名称返回错误。
    pub fn with_insecure_loopback_tls(mut self) -> Result<Self, DbError> {
        if self.tls_client_identity_pem.is_some() {
            return Err(DbError::Invalid {
                reason: "client identity cannot use insecure loopback TLS",
            });
        }
        if self.tls_ca_pem.is_some() {
            return Err(DbError::Invalid {
                reason: "custom CA cannot use insecure loopback TLS",
            });
        }
        if !self.host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()) {
            return Err(DbError::Invalid {
                reason: "insecure TLS requires literal loopback",
            });
        }
        self.insecure_loopback = true;
        Ok(self)
    }
    /// 参数为 1..65536 字节的 PEM CA 列表；返回保持 TLS 主机身份校验的配置。
    /// 配置阶段解析 PEM；证书结构、链及有效期在握手时由 SQLx/rustls 校验，禁止与关闭测试 TLS 混用。
    /// 此 CA 列表补充固定驱动内置的 WebPKI 信任根。
    /// 对应 Java: 原始 JDBC 数据源的 trustCertificateKeyStore 配置语义（部分实现）。
    pub fn with_tls_ca_pem(mut self, certificate: Vec<u8>) -> Result<Self, DbError> {
        if certificate.is_empty() || certificate.len() > 65536 {
            return Err(DbError::Invalid {
                reason: "TLS CA PEM byte budget",
            });
        }
        if self.insecure_loopback {
            return Err(DbError::Invalid {
                reason: "custom CA cannot use insecure loopback TLS",
            });
        }
        let mut found = false;
        for item in CertificateDer::pem_slice_iter(&certificate) {
            item.map_err(|_| DbError::Invalid {
                reason: "TLS CA PEM encoding",
            })?;
            found = true;
        }
        if !found {
            return Err(DbError::Invalid {
                reason: "TLS CA PEM requires a certificate",
            });
        }
        self.tls_ca_pem = Some(certificate);
        Ok(self)
    }
    /// 参数为各 1..65536 字节的 PEM 证书链与未加密私钥；返回保持服务端身份校验的配置。
    /// 配置阶段解析 PEM，DER 结构和密钥匹配由握手验证；禁止与关闭测试 TLS 混用。
    /// 对应 Java: 原始 JDBC 数据源的 clientCertificateKeyStore 配置语义（部分实现）。
    pub fn with_tls_client_identity_pem(
        mut self,
        certificate: Vec<u8>,
        private_key: Vec<u8>,
    ) -> Result<Self, DbError> {
        if certificate.is_empty()
            || certificate.len() > 65536
            || private_key.is_empty()
            || private_key.len() > 65536
        {
            return Err(DbError::Invalid {
                reason: "TLS client identity PEM byte budget",
            });
        }
        if self.insecure_loopback {
            return Err(DbError::Invalid {
                reason: "client identity cannot use insecure loopback TLS",
            });
        }
        let mut found = false;
        for item in CertificateDer::pem_slice_iter(&certificate) {
            item.map_err(|_| DbError::Invalid {
                reason: "TLS client certificate PEM encoding",
            })?;
            found = true;
        }
        if !found {
            return Err(DbError::Invalid {
                reason: "TLS client identity requires a certificate",
            });
        }
        PrivateKeyDer::from_pem_slice(&private_key).map_err(|_| DbError::Invalid {
            reason: "TLS client private key PEM encoding",
        })?;
        self.tls_client_identity_pem = Some((certificate, private_key));
        Ok(self)
    }
    /// 无参数；启用一阶段登记前的持久 JDBC 资源标识校验，返回更严格的配置。
    /// 对应 Java: 无直接对象；Rust 的 MySQL AT 部署扩展。
    pub fn require_at_resource_binding(mut self) -> Self {
        self.require_resource_binding = true;
        self
    }
    /// 无参数；要求原 JDBC 资源绑定行同时声明写入器的 TINYINT(1) 映射，返回更新后的配置。
    /// 部署方需应用 011 迁移并显式填写该行。事务数据源或准备管理器任一端启用即可要求校验。
    /// 对应 Java: 无直接对象；约束同一锁域的 Connector/J tinyInt1isBit 配置。
    pub fn require_at_tinyint_profile_binding(mut self) -> Self {
        self.require_resource_binding = true;
        self.require_tinyint_profile_binding = true;
        self
    }
    /// 无参数；启用固定 Java TC 原版 MySQL 锁表的登记前字符长度校验，返回更新后的配置。
    /// 业务事务数据源或准备管理器任一端启用即可要求校验。
    /// 对应 Java: 无直接对象；目标为 Apache Seata 原版 MySQL lock_table 的存储约束。
    pub fn with_java_tc_mysql_lock_limits(mut self) -> Self {
        self.java_tc_mysql_lock_limits = true;
        self
    }
    /// 无参数；要求业务文本主键使用固定 Java TC MySQL 锁表的 utf8mb4_0900_ai_ci。
    /// 业务事务数据源或准备管理器任一端启用时，均检查原业务事务上的列。
    /// 对应 Java: 无直接对象；Rust 的目标锁存储预检。
    pub fn with_java_tc_mysql_lock_collation(mut self) -> Self {
        self.java_tc_mysql_lock_collation = true;
        self
    }
    /// 无参数；为固定原版 Java TC MySQL DB 锁表同时启用资源绑定、字段长度和文本排序规则预检。
    /// 部署方须先应用 005 迁移并核对原 TC 的真实锁表定义、稳定单主与各写入器的原 JDBC ID。
    /// 对应 Java: 无直接对象；Rust 对固定 MySQL DB 锁存储的显式配置组合。
    pub fn with_java_tc_mysql_db_lock_profile(mut self) -> Self {
        self.require_resource_binding = true;
        self.java_tc_mysql_lock_limits = true;
        self.java_tc_mysql_lock_collation = true;
        self
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
        let options = MySqlConnectOptions::new()
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
            .log_slow_statements(log::LevelFilter::Off, Duration::from_secs(1));
        let options = match &self.tls_ca_pem {
            Some(certificate) => options.ssl_ca_from_pem(certificate.clone()),
            None => options,
        };
        match &self.tls_client_identity_pem {
            Some((certificate, private_key)) => options
                .ssl_client_cert_from_pem(certificate)
                .ssl_client_key_from_pem(private_key),
            None => options,
        }
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
            .field("require_resource_binding", &self.require_resource_binding)
            .field(
                "require_tinyint_profile_binding",
                &self.require_tinyint_profile_binding,
            )
            .field("java_tc_mysql_lock_limits", &self.java_tc_mysql_lock_limits)
            .field(
                "java_tc_mysql_lock_collation",
                &self.java_tc_mysql_lock_collation,
            )
            .field("maximum_connections", &self.maximum_connections)
            .finish_non_exhaustive()
    }
}
