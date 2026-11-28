use crate::codec::{WireReader, WireWriter};
use crate::ProtocolError;
use std::fmt;

/// RegisterRMRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.RegisterRMRequest
#[derive(Clone, PartialEq, Eq)]
pub struct RegisterRMRequest {
    /// 身份请求版本。完整V1消息要求非空，编解码时明确校验。
    pub version: String,
    /// 应用标识。
    pub application_id: Option<String>,
    /// 事务服务组。该Java解码分支将零长度设置为空串。
    pub transaction_service_group: String,
    /// 附加数据；按本消息的Java空值规则解释。
    pub extra_data: Option<String>,
    /// 资源集合文本，Java零长度解码为None。
    pub resource_ids: Option<String>,
}

impl fmt::Debug for RegisterRMRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("RegisterRMRequest");
        debug.field("version_bytes", &self.version.len());
        debug.field("application_id_present", &self.application_id.is_some());
        debug.field(
            "transaction_service_group_bytes",
            &self.transaction_service_group.len(),
        );
        debug.field("extra_data_present", &self.extra_data.is_some());
        debug.field("resource_ids_present", &self.resource_ids.is_some());
        debug.finish_non_exhaustive()
    }
}

impl RegisterRMRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            version: reader
                .string16("version")?
                .ok_or(ProtocolError::InvalidValue {
                    field: "version",
                    reason: "complete message requires a nonempty field",
                })?,
            application_id: reader.string16("application_id")?,
            transaction_service_group: reader
                .string16("transaction_service_group")?
                .unwrap_or_default(),
            extra_data: reader.string16("extra_data")?,
            resource_ids: reader.string32("resource_ids")?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        if self.version.is_empty() {
            return Err(ProtocolError::InvalidValue {
                field: "version",
                reason: "complete message requires a nonempty field",
            });
        }
        writer.string16(Some(&self.version), "version")?;
        writer.string16(self.application_id.as_deref(), "application_id")?;
        writer.string16(
            Some(&self.transaction_service_group),
            "transaction_service_group",
        )?;
        writer.string16(self.extra_data.as_deref(), "extra_data")?;
        writer.string32(self.resource_ids.as_deref(), "resource_ids")?;
        Ok(())
    }
}
