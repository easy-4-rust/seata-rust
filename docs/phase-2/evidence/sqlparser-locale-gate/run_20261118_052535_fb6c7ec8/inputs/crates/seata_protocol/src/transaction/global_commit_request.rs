use crate::codec::{WireReader, WireWriter};
use crate::ProtocolError;
use std::fmt;

/// GlobalCommitRequest的原生V1字段模型；Debug不展示字符串内容。
/// 对应 Java: org.apache.seata.core.protocol.transaction.GlobalCommitRequest
#[derive(Clone, PartialEq, Eq)]
pub struct GlobalCommitRequest {
    /// 全局事务标识。
    pub xid: Option<String>,
    /// 附加数据；按本消息的Java空值规则解释。
    pub extra_data: Option<String>,
}

impl fmt::Debug for GlobalCommitRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("GlobalCommitRequest");
        debug.field("xid_present", &self.xid.is_some());
        debug.field("extra_data_present", &self.extra_data.is_some());
        debug.finish_non_exhaustive()
    }
}

impl GlobalCommitRequest {
    /// 从已定位消息体的游标读取完整字段；截断/非法码等返回错误，不恢复游标。
    pub(crate) fn read(reader: &mut WireReader<'_>) -> Result<Self, ProtocolError> {
        Ok(Self {
            xid: reader.string16("xid")?,
            extra_data: reader.string16("extra_data")?,
        })
    }

    /// 按本消息的Java V1字段顺序写入；不添加类型前缀，失败由上层丢弃整体输出。
    pub(crate) fn write(&self, writer: &mut WireWriter) -> Result<(), ProtocolError> {
        writer.string16(self.xid.as_deref(), "xid")?;
        writer.string16(self.extra_data.as_deref(), "extra_data")?;
        Ok(())
    }
}
