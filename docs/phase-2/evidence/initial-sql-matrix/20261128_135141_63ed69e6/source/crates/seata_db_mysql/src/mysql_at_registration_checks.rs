/// 业务连接池和准备管理器各自要求的登记前校验。对应 Java: 无直接对象，Rust 部署扩展。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MySqlAtRegistrationChecks {
    pub(crate) require_resource_binding: bool,
    pub(crate) require_tinyint_profile_binding: bool,
    pub(crate) require_jdbc_profile_binding: bool,
    pub(crate) java_tc_mysql_lock_limits: bool,
    pub(crate) java_tc_mysql_lock_collation: bool,
}
