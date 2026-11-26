use crate::codec::{WireReader, WireWriter};
use crate::ProtocolError;
use std::fmt;

/// RegisterTMRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.RegisterTMRequest
#[derive(Clone, PartialEq, Eq)]
pub struct RegisterTMRequest {
    /// 身份请求版本。该Java解码分支将零长度设置为空串。
    pub version: String,
    /// 应用标识。该Java解码分支将零长度设置为空串。
    pub application_id: String,
    /// 事务服务组。该Java解码分支将零长度设置为空串。
    pub transaction_service_group: String,
    /// 附加数据；按本消息的Java空值规则解释。该Java解码分支将零长度设置为空串。
    pub extra_data: String,
}

impl fmt::Debug for RegisterTMRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("RegisterTMRequest");
        debug.field("version_bytes", &self.version.len());
        debug.field("application_id_bytes", &self.application_id.len());
        debug.field(
            "transaction_service_group_bytes",
            &self.transaction_service_group.len(),
        );
        debug.field("extra_data_bytes", &self.extra_data.len());
        debug.finish_non_exhaustive()
    }
}

impl RegisterTMRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            version: reader.string16("version")?.unwrap_or_default(),
            application_id: reader.string16("application_id")?.unwrap_or_default(),
            transaction_service_group: reader
                .string16("transaction_service_group")?
                .unwrap_or_default(),
            extra_data: reader.string16("extra_data")?.unwrap_or_default(),
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        writer.string16(Some(&self.version), "version")?;
        writer.string16(Some(&self.application_id), "application_id")?;
        writer.string16(
            Some(&self.transaction_service_group),
            "transaction_service_group",
        )?;
        writer.string16(Some(&self.extra_data), "extra_data")?;
        Ok(())
    }
}
