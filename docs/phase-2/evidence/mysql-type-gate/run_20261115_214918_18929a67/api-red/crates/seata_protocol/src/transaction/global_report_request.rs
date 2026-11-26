use crate::codec::{WireReader, WireWriter};
use crate::{GlobalStatus, ProtocolError};
use std::fmt;

/// GlobalReportRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.GlobalReportRequest
#[derive(Clone, PartialEq, Eq)]
pub struct GlobalReportRequest {
    /// 全局事务标识。
    pub xid: Option<String>,
    /// 附加数据；按本消息的Java空值规则解释。
    pub extra_data: Option<String>,
    /// 全局状态，不替代响应结果码。255代表缺省None，0代表Some(Unknown)。
    pub global_status: Option<GlobalStatus>,
}

impl fmt::Debug for GlobalReportRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("GlobalReportRequest");
        debug.field("xid_present", &self.xid.is_some());
        debug.field("extra_data_present", &self.extra_data.is_some());
        debug.field("global_status", &self.global_status);
        debug.finish_non_exhaustive()
    }
}

impl GlobalReportRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            xid: reader.string16("xid")?,
            extra_data: reader.string16("extra_data")?,
            global_status: {
                let code = reader.u8()?;
                if code == 255 {
                    None
                } else {
                    Some(GlobalStatus::from_java_v1(code)?)
                }
            },
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        writer.string16(self.xid.as_deref(), "xid")?;
        writer.string16(self.extra_data.as_deref(), "extra_data")?;
        writer.u8(self.global_status.map_or(255, GlobalStatus::to_java_v1))?;
        Ok(())
    }
}
