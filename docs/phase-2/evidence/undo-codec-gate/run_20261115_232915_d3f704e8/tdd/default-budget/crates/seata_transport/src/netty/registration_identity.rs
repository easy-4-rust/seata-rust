use crate::TransportError;
/// 原生会话登记的显式身份字段；Debug只展示长度/存在性，不输出凭据或应用标识。
/// 对应 Java: AbstractIdentifyRequest的版本/应用/服务组/extraData。
#[derive(Clone)]
pub struct RegistrationIdentity {
    pub(crate) version: String,
    pub(crate) application_id: String,
    pub(crate) transaction_service_group: String,
    pub(crate) extra_data: Option<String>,
}
impl RegistrationIdentity {
    /// 参数是版本、应用、服务组和可选附加数据；返回可由原生string16完整表示的配置。
    pub fn new(
        version: String,
        application_id: String,
        transaction_service_group: String,
        extra_data: Option<String>,
    ) -> Result<Self, TransportError> {
        if [&version, &application_id, &transaction_service_group]
            .iter()
            .any(|value| value.is_empty() || value.len() > 32767)
            || extra_data.as_ref().is_some_and(|value| value.len() > 32767)
        {
            return Err(TransportError::InvalidConfiguration {
                reason: "nonempty identity fields and bounded native string16 required",
            });
        }
        Ok(Self {
            version,
            application_id,
            transaction_service_group,
            extra_data,
        })
    }
}
impl std::fmt::Debug for RegistrationIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistrationIdentity")
            .field("version_bytes", &self.version.len())
            .field("application_id_bytes", &self.application_id.len())
            .field("group_bytes", &self.transaction_service_group.len())
            .field("extra_data_present", &self.extra_data.is_some())
            .finish()
    }
}
